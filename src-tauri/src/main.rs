// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Agent hooks and the Claude Code status line run this same executable;
    // handle them and exit before any window or webview gets created.
    if termini_lib::run_cli_mode_from_args() {
        return;
    }
    termini_lib::run()
}
