//! Starting up: each console window shows once it's whole, and how long that
//! took is written down.
//!
//! A console window is built hidden (tauri.conf.json), at the size it was
//! left at, and its page says when its tabs are in place (`window_ready`):
//! it shows then, at once, instead of assembling itself on screen — the
//! default size, theme and language first, then the saved ones, then the
//! consoles. Never half built: the page says so even when restoring its tabs
//! fails, so the grace period is only for a page that never runs at all.
//!
//! The first window's timings, from the process starting to it showing, go
//! to `startup.log` in the app's log folder, one line per launch.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::config::schema::WindowConfig;

/// How long a window waits for its page before showing regardless: well past
/// any start, however slow, so it never shows one half done.
const READY_GRACE: Duration = Duration::from_secs(15);
/// Launches kept in startup.log.
const LOG_LINES: usize = 50;

static START: OnceLock<Instant> = OnceLock::new();
/// Milliseconds since `begin`, by name, until the first window shows.
static MARKS: Mutex<Vec<(String, u128)>> = Mutex::new(Vec::new());
static LOGGED: AtomicBool = AtomicBool::new(false);

/// The clock starts: first thing in `run`.
pub fn begin() {
    START.get_or_init(Instant::now);
}

/// One more point on the way to the first window showing.
pub fn mark(name: &str) {
    let Some(start) = START.get() else { return };
    if LOGGED.load(Ordering::SeqCst) {
        return;
    }
    MARKS.lock().unwrap().push((name.to_string(), start.elapsed().as_millis()));
}

/// A console window just built, hidden: it takes the size it was left at
/// before its page lays anything out, and shows by itself if its page never
/// reports ready.
pub fn hold(window: &tauri::WebviewWindow, size: &WindowConfig) {
    if size.width >= 1.0 && size.height >= 1.0 {
        let _ = window.set_size(tauri::PhysicalSize::new(size.width as u32, size.height as u32));
    }
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_GRACE);
        if !window.is_visible().unwrap_or(true) {
            mark("grace");
            reveal(&window);
        }
    });
}

/// The window's page has its tabs in place: it shows. `page_marks` are the
/// page's own timings, in milliseconds since it started loading.
pub fn ready(window: &tauri::WebviewWindow, page_marks: Vec<(String, f64)>) {
    if !window.is_visible().unwrap_or(true) {
        reveal(window);
    }
    if window.label() == crate::terminal_windows::MAIN {
        mark("shown");
        if !LOGGED.swap(true, Ordering::SeqCst) {
            write_log(window.app_handle(), &page_marks);
        }
    }
}

fn reveal(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

/// `setup=12 … shown=790 | page: config=80 … ready=180`: the process's
/// marks since it started, then the page's since it started loading.
fn log_line(marks: &[(String, u128)], page_marks: &[(String, f64)]) -> String {
    let process = marks.iter().map(|(name, ms)| format!("{name}={ms}")).collect::<Vec<_>>().join(" ");
    let page = page_marks.iter().map(|(name, ms)| format!("{name}={}", ms.round() as i64)).collect::<Vec<_>>().join(" ");
    format!("{process} | page: {page}")
}

fn write_log(app: &tauri::AppHandle, page_marks: &[(String, f64)]) {
    let line = format!(
        "{} v{}{} {} (ms)",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        app.package_info().version,
        if cfg!(debug_assertions) { " dev" } else { "" },
        log_line(&MARKS.lock().unwrap(), page_marks),
    );
    #[cfg(debug_assertions)]
    eprintln!("startup: {line}");
    let Ok(dir) = app.path().app_log_dir() else { return };
    // Off the window's way: nothing waits for the log.
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("startup.log");
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        let mut lines: Vec<&str> = old.lines().collect();
        lines.push(&line);
        let kept = &lines[lines.len().saturating_sub(LOG_LINES)..];
        let _ = std::fs::write(&path, kept.join("\n") + "\n");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_line_has_the_process_marks_then_the_page_ones() {
        let marks = vec![("setup".to_string(), 12), ("shown".to_string(), 790)];
        let page = vec![("config".to_string(), 80.4), ("ready".to_string(), 179.6)];
        assert_eq!(log_line(&marks, &page), "setup=12 shown=790 | page: config=80 ready=180");
    }
}
