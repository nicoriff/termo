//! Resolves a PID to its executable's full resolved path. Unix-only — there
//! is no portable-pty equivalent of process_group_leader() on the Windows
//! ConPTY backend, so this module is never compiled/called on Windows and
//! the foreground-process signal is simply unavailable there; Modo IA falls
//! back to text-pattern detection alone in that case.
//!
//! Returns the full path rather than a bare basename deliberately: Claude
//! Code's own installer symlinks a stable `claude` name to a version-pinned
//! binary (e.g. `~/.local/share/claude/versions/2.1.269`), so the resolved
//! executable's basename is a version string, not "claude" — matching has
//! to look at the whole path instead (see pattern-table.ts's processNames).

#[cfg(target_os = "linux")]
pub fn process_path(pid: libc::pid_t) -> Option<String> {
    // /proc/<pid>/comm truncates to 15 bytes, which cuts off many real
    // binary names — the /exe symlink's target has no such limit.
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
pub fn process_path(pid: libc::pid_t) -> Option<String> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut std::ffi::c_void, buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    Some(String::from_utf8_lossy(&buf).into_owned())
}
