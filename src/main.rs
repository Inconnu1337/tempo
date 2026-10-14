#![windows_subsystem = "windows"]

mod common;
mod icon;
mod tracker;
mod ui;
mod winutil;

use common::*;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "--background" => tracker::run(),
        "--ui" => ui::run(),
        _ => {
            let mut cfg = Config::load();
            if !cfg.first_run_done {
                cfg.first_run_done = true;
                cfg.save();
                autostart::set(true);
            } else if autostart::is_enabled() {
                autostart::set(true);
            }
            if !winutil::tracker_running() {
                winutil::start_tracker();
            }
            ui::run();
        }
    }
}
