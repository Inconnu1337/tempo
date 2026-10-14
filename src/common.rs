use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub const TRACKER_CLASS: &str = "TempoTrackerWnd";
pub const UI_TITLE: &str = "Tempo";
pub const WM_TRAY: u32 = 0x8000 + 1; // WM_APP + 1
pub const WM_RELOAD: u32 = 0x8000 + 2;
pub const WM_FLUSH: u32 = 0x8000 + 3;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join("Tempo");
    let _ = fs::create_dir_all(d.join("days"));
    d
}

pub fn write_atomic(path: &PathBuf, content: &str) {
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, content).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Date {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

impl Date {
    pub fn today() -> Date {
        let st = local_time();
        Date { y: st.wYear as i32, m: st.wMonth as u32, d: st.wDay as u32 }
    }
    pub fn to_days(self) -> i64 {
        let y = if self.m <= 2 { self.y - 1 } else { self.y } as i64;
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.m as i64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + self.d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }
    pub fn from_days(z: i64) -> Date {
        let z = z + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        Date { y: (if m <= 2 { y + 1 } else { y }) as i32, m, d }
    }
    pub fn add(self, n: i64) -> Date {
        Date::from_days(self.to_days() + n)
    }
    /// 0 = Monday
    pub fn weekday(self) -> u32 {
        ((self.to_days() + 3).rem_euclid(7)) as u32
    }
    pub fn key(self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
    pub fn parse(s: &str) -> Option<Date> {
        let mut it = s.split('-');
        Some(Date {
            y: it.next()?.parse().ok()?,
            m: it.next()?.parse().ok()?,
            d: it.next()?.parse().ok()?,
        })
    }
}

pub fn local_hour() -> usize {
    local_time().wHour as usize
}

fn local_time() -> windows_sys::Win32::Foundation::SYSTEMTIME {
    let mut st = unsafe { std::mem::zeroed() };
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut st) };
    st
}

#[derive(Clone)]
pub struct Config {
    pub idle_secs: u64,
    pub paused: bool,
    pub excluded: Vec<String>,
    pub first_run_done: bool,
}

pub const DEFAULT_EXCLUDED: &[&str] = &[
    "applicationframehost.exe",
    "searchhost.exe",
    "searchapp.exe",
    "searchui.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "shellhost.exe",
    "lockapp.exe",
    "logonui.exe",
    "textinputhost.exe",
    "dwm.exe",
    "csrss.exe",
    "winlogon.exe",
    "consent.exe",
    "sihost.exe",
    "ctfmon.exe",
    "svchost.exe",
    "taskhostw.exe",
    "runtimebroker.exe",
    "fontdrvhost.exe",
    "smartscreen.exe",
    "rundll32.exe",
    "dllhost.exe",
    "pickerhost.exe",
    "credentialuibroker.exe",
    "systemsettingsbroker.exe",
    "useroobebroker.exe",
    "windowsinternal.composableshell.experiences.textinput.inputapp.exe",
    "tempo.exe",
];

impl Default for Config {
    fn default() -> Self {
        Config {
            idle_secs: 300,
            paused: false,
            excluded: DEFAULT_EXCLUDED.iter().map(|s| s.to_string()).collect(),
            first_run_done: false,
        }
    }
}

impl Config {
    fn path() -> PathBuf {
        data_dir().join("config.txt")
    }
    pub fn load() -> Config {
        let Ok(text) = fs::read_to_string(Self::path()) else {
            return Config::default();
        };
        let mut c = Config { excluded: Vec::new(), ..Config::default() };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "idle" => c.idle_secs = v.trim().parse().unwrap_or(300).clamp(30, 7200),
                "paused" => c.paused = v.trim() == "1",
                "first_run_done" => c.first_run_done = v.trim() == "1",
                "exclude" => {
                    let e = v.trim().to_lowercase();
                    if !e.is_empty() && !c.excluded.contains(&e) {
                        c.excluded.push(e);
                    }
                }
                _ => {}
            }
        }
        c
    }
    pub fn save(&self) {
        let mut s = format!(
            "idle={}\npaused={}\nfirst_run_done={}\n",
            self.idle_secs, self.paused as u8, self.first_run_done as u8
        );
        for e in &self.excluded {
            s.push_str("exclude=");
            s.push_str(e);
            s.push('\n');
        }
        write_atomic(&Self::path(), &s);
    }
    pub fn is_excluded(&self, exe: &str) -> bool {
        self.excluded.iter().any(|e| e == exe)
    }
}


#[derive(Default, Clone)]
pub struct DayData {
    pub apps: HashMap<String, u64>,
    pub hours: [u64; 24],
}

impl DayData {
    pub fn path(date: Date) -> PathBuf {
        data_dir().join("days").join(format!("{}.tsv", date.key()))
    }
    pub fn load(date: Date) -> DayData {
        let mut d = DayData::default();
        let Ok(text) = fs::read_to_string(Self::path(date)) else { return d };
        for line in text.lines() {
            let mut it = line.split('\t');
            match (it.next(), it.next(), it.next()) {
                (Some("A"), Some(exe), Some(secs)) => {
                    *d.apps.entry(exe.to_string()).or_default() += secs.parse::<u64>().unwrap_or(0);
                }
                (Some("H"), Some(h), Some(secs)) => {
                    if let Ok(h) = h.parse::<usize>()
                        && h < 24 {
                            d.hours[h] += secs.parse::<u64>().unwrap_or(0);
                        }
                }
                _ => {}
            }
        }
        d
    }
    pub fn save(&self, date: Date) {
        let mut s = String::with_capacity(64 * self.apps.len() + 256);
        for (exe, secs) in &self.apps {
            s.push_str(&format!("A\t{exe}\t{secs}\n"));
        }
        for (h, secs) in self.hours.iter().enumerate() {
            if *secs > 0 {
                s.push_str(&format!("H\t{h}\t{secs}\n"));
            }
        }
        write_atomic(&Self::path(date), &s);
    }
    pub fn total(&self) -> u64 {
        self.apps.values().sum()
    }
}

pub fn load_app_paths() -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(text) = fs::read_to_string(data_dir().join("apps.tsv")) {
        for line in text.lines() {
            if let Some((exe, path)) = line.split_once('\t') {
                m.insert(exe.to_string(), path.to_string());
            }
        }
    }
    m
}

pub fn save_app_paths(m: &HashMap<String, String>) {
    let mut s = String::new();
    for (exe, path) in m {
        s.push_str(&format!("{exe}\t{path}\n"));
    }
    write_atomic(&data_dir().join("apps.tsv"), &s);
}

pub fn fmt_dur(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h} ч {m:02} мин")
    } else if m > 0 {
        format!("{m} мин")
    } else {
        format!("{secs} с")
    }
}


pub mod autostart {
    use super::wide;
    use windows_sys::Win32::System::Registry::*;

    const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const NAME: &str = "Tempo";

    fn open() -> Option<HKEY> {
        let mut key: HKEY = std::ptr::null_mut();
        let r = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN).as_ptr(), 0, KEY_READ | KEY_WRITE, &mut key)
        };
        (r == 0).then_some(key)
    }

    pub fn is_enabled() -> bool {
        let Some(k) = open() else { return false };
        let mut size = 0u32;
        let r = unsafe {
            RegQueryValueExW(k, wide(NAME).as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), &mut size)
        };
        unsafe { RegCloseKey(k) };
        r == 0
    }

    pub fn set(enabled: bool) {
        let Some(k) = open() else { return };
        unsafe {
            if enabled {
                let exe = std::env::current_exe().unwrap_or_default();
                let cmd = wide(&format!("\"{}\" --background", exe.display()));
                RegSetValueExW(k, wide(NAME).as_ptr(), 0, REG_SZ, cmd.as_ptr() as *const u8, (cmd.len() * 2) as u32);
            } else {
                RegDeleteValueW(k, wide(NAME).as_ptr());
            }
            RegCloseKey(k);
        }
    }
}
