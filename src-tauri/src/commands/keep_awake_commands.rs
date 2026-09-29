use tauri::State;

use crate::keep_awake::KeepAwakeState;
use crate::state::AppState;

#[tauri::command]
pub fn keep_awake_state(state: State<'_, AppState>) -> KeepAwakeState {
    state.keep_awake.state()
}

/// Turns keeping the computer awake on (for `minutes`, or until turned off
/// when none) or off. Every window hears the new state as `keep-awake`.
#[tauri::command]
pub fn keep_awake_set(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    active: bool,
    display: bool,
    minutes: Option<u32>,
) -> Result<KeepAwakeState, String> {
    state.keep_awake.set(&app, active, display, minutes)
}
