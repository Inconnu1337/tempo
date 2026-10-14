use crate::common::*;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn tracker_hwnd() -> HWND {
    unsafe { FindWindowW(wide(TRACKER_CLASS).as_ptr(), null_mut()) }
}

pub fn tracker_running() -> bool {
    !tracker_hwnd().is_null()
}

pub fn start_tracker() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--background").spawn();
    }
}

pub fn stop_tracker() {
    let h = tracker_hwnd();
    if !h.is_null() {
        unsafe { PostMessageW(h, WM_CLOSE, 0, 0) };
    }
}

pub fn notify_reload() {
    let h = tracker_hwnd();
    if !h.is_null() {
        unsafe { PostMessageW(h, WM_RELOAD, 0, 0) };
    }
}

pub fn flush_tracker() {
    let h = tracker_hwnd();
    if !h.is_null() {
        let mut res = 0usize;
        unsafe { SendMessageTimeoutW(h, WM_FLUSH, 0, 0, SMTO_ABORTIFHUNG, 300, &mut res) };
    }
}

pub fn focus_existing_ui() -> bool {
    unsafe {
        let h = FindWindowW(null_mut(), wide(UI_TITLE).as_ptr());
        if h.is_null() {
            return false;
        }
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h);
        true
    }
}

pub fn open_path(p: &str) {
    unsafe {
        ShellExecuteW(null_mut(), wide("open").as_ptr(), wide(p).as_ptr(), null_mut(), null_mut(), SW_SHOWNORMAL);
    }
}

pub fn app_description(path: &str) -> Option<String> {
    unsafe {
        let wpath = wide(path);
        let size = GetFileVersionInfoSizeW(wpath.as_ptr(), null_mut());
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wpath.as_ptr(), 0, size, buf.as_mut_ptr() as _) == 0 {
            return None;
        }
        let mut ptr: *mut core::ffi::c_void = null_mut();
        let mut len = 0u32;
        let mut langs: Vec<(u16, u16)> = Vec::new();
        if VerQueryValueW(buf.as_ptr() as _, wide("\\VarFileInfo\\Translation").as_ptr(), &mut ptr, &mut len) != 0 && len >= 4 {
            let n = len as usize / 4;
            let s = std::slice::from_raw_parts(ptr as *const u16, n * 2);
            for i in 0..n {
                langs.push((s[i * 2], s[i * 2 + 1]));
            }
        }
        langs.extend([(0x0409, 0x04B0), (0x0409, 0x04E4), (0x0419, 0x04B0)]);
        for (lang, cp) in langs {
            let key = format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription");
            if VerQueryValueW(buf.as_ptr() as _, wide(&key).as_ptr(), &mut ptr, &mut len) != 0 && len > 1 {
                let s = std::slice::from_raw_parts(ptr as *const u16, len as usize);
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                let d = String::from_utf16_lossy(&s[..end]).trim().to_string();
                if !d.is_empty() {
                    return Some(d);
                }
            }
        }
        None
    }
}

pub fn app_icon_rgba(path: &str) -> Option<(usize, usize, Vec<u8>)> {
    unsafe {
        let mut sfi: SHFILEINFOW = std::mem::zeroed();
        let r = SHGetFileInfoW(wide(path).as_ptr(), 0, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
        if r == 0 || sfi.hIcon.is_null() {
            return None;
        }
        let out = hicon_to_rgba(sfi.hIcon);
        DestroyIcon(sfi.hIcon);
        out
    }
}

unsafe fn bitmap_bits(dc: HDC, bmp: HBITMAP, w: i32, h: i32) -> Option<Vec<u8>> {
    let mut bi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = w;
    bi.bmiHeader.biHeight = -h;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    let mut px = vec![0u8; (w * h * 4) as usize];
    let n = unsafe { GetDIBits(dc, bmp, 0, h as u32, px.as_mut_ptr() as _, &mut bi, DIB_RGB_COLORS) };
    (n != 0).then_some(px)
}

unsafe fn hicon_to_rgba(icon: HICON) -> Option<(usize, usize, Vec<u8>)> {
    unsafe {
        let mut ii: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon, &mut ii) == 0 {
            return None;
        }
        let mut bm: BITMAP = std::mem::zeroed();
        let res = (|| {
            if ii.hbmColor.is_null() {
                return None;
            }
            GetObjectW(ii.hbmColor, std::mem::size_of::<BITMAP>() as i32, &mut bm as *mut _ as _);
            let (w, h) = (bm.bmWidth, bm.bmHeight);
            if w <= 0 || h <= 0 || w > 256 || h > 256 {
                return None;
            }
            let dc = CreateCompatibleDC(null_mut());
            let color = bitmap_bits(dc, ii.hbmColor, w, h);
            let mask = if ii.hbmMask.is_null() { None } else { bitmap_bits(dc, ii.hbmMask, w, h) };
            DeleteDC(dc);
            let mut px = color?;
            let has_alpha = px.chunks(4).any(|c| c[3] != 0);
            for (i, c) in px.chunks_mut(4).enumerate() {
                c.swap(0, 2); // BGRA to RGBA
                if !has_alpha {
                    let transparent = mask.as_ref().map(|m| m[i * 4] != 0).unwrap_or(false);
                    c[3] = if transparent { 0 } else { 255 };
                }
            }
            Some((w as usize, h as usize, px))
        })();
        if !ii.hbmColor.is_null() {
            DeleteObject(ii.hbmColor);
        }
        if !ii.hbmMask.is_null() {
            DeleteObject(ii.hbmMask);
        }
        res
    }
}

pub fn windows_font(name: &str) -> Option<Vec<u8>> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    std::fs::read(format!("{windir}\\Fonts\\{name}")).ok()
}
