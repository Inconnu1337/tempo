use crate::common::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TICK_MS: u32 = 2000;
const TIMER_TICK: usize = 1;
const FLUSH_EVERY_TICKS: u32 = 30; // ~60 s
const MENU_OPEN: usize = 100;
const MENU_PAUSE: usize = 101;
const MENU_EXIT: usize = 102;

struct State {
    cfg: Config,
    date: Date,
    day: DayData,
    dirty: bool,
    paths: HashMap<String, String>,
    paths_dirty: bool,
    last_tick: u64,
    ticks: u32,
    last_hwnd: HWND,
    last_pid: u32,
    last_exe: Option<String>,
    icon: HICON,
    taskbar_created: u32,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.borrow_mut().as_mut().map(f))
}

pub fn run() {
    unsafe {
        if !FindWindowW(wide(TRACKER_CLASS).as_ptr(), null_mut()).is_null() {
            return;
        }
        let hinst = GetModuleHandleW(null_mut());
        let class = wide(TRACKER_CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            wide("Tempo tracker").as_ptr(),
            WS_POPUP,
            0, 0, 0, 0,
            null_mut(), null_mut(), hinst, null_mut(),
        );
        if hwnd.is_null() {
            return;
        }

        let payload = crate::icon::icon_bmp_payload(32);
        let icon = CreateIconFromResourceEx(payload.as_ptr(), payload.len() as u32, 1, 0x0003_0000, 32, 32, 0);

        let date = Date::today();
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                cfg: Config::load(),
                date,
                day: DayData::load(date),
                dirty: false,
                paths: load_app_paths(),
                paths_dirty: false,
                last_tick: GetTickCount64(),
                ticks: 0,
                last_hwnd: null_mut(),
                last_pid: 0,
                last_exe: None,
                icon,
                taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            })
        });

        add_tray(hwnd);
        SetTimer(hwnd, TIMER_TICK, TICK_MS, None);
        trim_memory();

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        flush(true);
        remove_tray(hwnd);
    }
}

fn trim_memory() {
    unsafe {
        SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid
}

fn set_tip(nid: &mut NOTIFYICONDATAW) {
    let tip = with(|s| {
        if s.cfg.paused {
            "Tempo: отслеживание поставлено на паузу.".to_string()
        } else {
            format!("Tempo: сегодня вы были активны {}.", fmt_dur(s.day.total()))
        }
    })
    .unwrap_or_else(|| "Tempo".into());
    let w: Vec<u16> = tip.encode_utf16().take(127).collect();
    nid.szTip = [0; 128];
    nid.szTip[..w.len()].copy_from_slice(&w);
}

fn add_tray(hwnd: HWND) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = with(|s| s.icon).unwrap_or(null_mut());
    set_tip(&mut nid);
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
}

fn update_tip(hwnd: HWND) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_TIP;
    set_tip(&mut nid);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
}

fn remove_tray(hwnd: HWND) {
    let nid = tray_data(hwnd);
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

fn flush(force: bool) {
    with(|s| {
        if s.dirty || force {
            s.day.save(s.date);
            s.dirty = false;
        }
        if s.paths_dirty {
            save_app_paths(&s.paths);
            s.paths_dirty = false;
        }
    });
}

fn open_ui() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--ui").spawn();
    }
}

fn idle_secs() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut lii) == 0 {
            return 0;
        }
        let now = windows_sys::Win32::System::SystemInformation::GetTickCount();
        (now.wrapping_sub(lii.dwTime) / 1000) as u64
    }
}

fn is_fullscreen(hwnd: HWND) -> bool {
    unsafe {
        let mut wr: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut wr) == 0 {
            return false;
        }
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return false;
        }
        let m = mi.rcMonitor;
        wr.left <= m.left && wr.top <= m.top && wr.right >= m.right && wr.bottom >= m.bottom
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn uwp_child_pid(hwnd: HWND, host_pid: u32) -> Option<u32> {
    struct Ctx {
        host: u32,
        found: u32,
    }
    unsafe extern "system" fn cb(child: HWND, lp: LPARAM) -> windows_sys::core::BOOL {
        let ctx = unsafe { &mut *(lp as *mut Ctx) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(child, &mut pid) };
        if pid != 0 && pid != ctx.host {
            ctx.found = pid;
            return 0;
        }
        1
    }
    let mut ctx = Ctx { host: host_pid, found: 0 };
    unsafe { EnumChildWindows(hwnd, Some(cb), &mut ctx as *mut Ctx as LPARAM) };
    (ctx.found != 0).then_some(ctx.found)
}

fn foreground_app(s: &mut State) -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if hwnd == s.last_hwnd && pid == s.last_pid {
            return s.last_exe.clone();
        }
        s.last_hwnd = hwnd;
        s.last_pid = pid;
        s.last_exe = None;

        let cls = class_name(hwnd);
        if matches!(cls.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "NotifyIconOverflowWindow" | "TopLevelWindowForOverflowXamlIsland" | "XamlExplorerHostIslandWindow") {
            return None;
        }
        let mut path = process_path(pid)?;
        let lower = path.to_lowercase();
        if lower.ends_with("\\applicationframehost.exe") {
            {
                let p = uwp_child_pid(hwnd, pid).and_then(process_path)?;
                path = p;
            }
        }
        let exe = path.rsplit('\\').next().unwrap_or(&path).to_lowercase();
        if !s.paths.contains_key(&exe) {
            s.paths.insert(exe.clone(), path);
            s.paths_dirty = true;
        }
        s.last_exe = Some(exe.clone());
        Some(exe)
    }
}

fn tick(hwnd: HWND) {
    let now = unsafe { GetTickCount64() };
    let need_tip = with(|s| {
        let dt_ms = now.saturating_sub(s.last_tick);
        s.last_tick = now;
        let dt = if dt_ms > (TICK_MS as u64) * 3 { 0 } else { dt_ms };

        let today = Date::today();
        if today != s.date {
            s.day.save(s.date);
            s.date = today;
            s.day = DayData::load(today);
            s.dirty = false;
        }

        if !s.cfg.paused && dt > 0 {
            let fg = unsafe { GetForegroundWindow() };
            let idle = idle_secs() >= s.cfg.idle_secs && !(!fg.is_null() && is_fullscreen(fg));
            if !idle
                && let Some(exe) = foreground_app(s)
                    && !s.cfg.is_excluded(&exe) {
                        let e = s.day.apps.entry(exe).or_default();
                        let h = local_hour();
                        let acc = REM.with(|r| {
                            let mut r = r.borrow_mut();
                            *r += dt;
                            let whole = *r / 1000;
                            *r %= 1000;
                            whole
                        });
                        *e += acc;
                        s.day.hours[h] += acc;
                        if acc > 0 {
                            s.dirty = true;
                        }
                    }
        }
        s.ticks += 1;
        s.ticks % FLUSH_EVERY_TICKS == 0
    })
    .unwrap_or(false);
    if need_tip {
        flush(false);
        update_tip(hwnd);
        trim_memory();
    }
}

thread_local! {
    static REM: RefCell<u64> = const { RefCell::new(0) };
}

fn show_menu(hwnd: HWND) {
    unsafe {
        let paused = with(|s| s.cfg.paused).unwrap_or(false);
        let m = CreatePopupMenu();
        AppendMenuW(m, MF_STRING, MENU_OPEN, wide("Открыть статистику").as_ptr());
        AppendMenuW(m, MF_STRING, MENU_PAUSE, wide(if paused { "Продолжить отслеживание" } else { "Поставить отслеживание на паузу" }).as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null_mut());
        AppendMenuW(m, MF_STRING, MENU_EXIT, wide("Закрыть трекер").as_ptr());
        SetMenuDefaultItem(m, MENU_OPEN as u32, 0);
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null_mut());
        DestroyMenu(m);
        match cmd as usize {
            MENU_OPEN => open_ui(),
            MENU_PAUSE => {
                with(|s| {
                    s.cfg = Config::load();
                    s.cfg.paused = !s.cfg.paused;
                    s.cfg.save();
                });
                update_tip(hwnd);
            }
            MENU_EXIT => {
                DestroyWindow(hwnd);
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER if wp == TIMER_TICK => {
            tick(hwnd);
            0
        }
        WM_TRAY => {
            match (lp as u32) & 0xFFFF {
                WM_LBUTTONUP => open_ui(),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_RELOAD => {
            with(|s| s.cfg = Config::load());
            update_tip(hwnd);
            0
        }
        WM_FLUSH => {
            flush(false);
            1
        }
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            flush(true);
            0
        }
        WM_POWERBROADCAST => {
            if wp == PBT_APMSUSPEND as usize {
                flush(true);
            }
            1
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            if with(|s| s.taskbar_created).unwrap_or(0) == msg && msg != 0 {
                add_tray(hwnd);
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
        }
    }
}
