// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Agent hooks run this same executable; handle the event and exit
    // before any window or webview gets created.
    if termini_lib::run_agent_hook_from_args() {
        return;
    }
    termini_lib::run()
}
