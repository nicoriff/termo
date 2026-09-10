//! Native rounded-corner window shaping for Windows.
//!
//! CSS `border-radius` on the webview's root element only clips the
//! WebView2 surface's own painting — in practice this does not reliably
//! propagate to how DWM composites the real HWND against the desktop, so
//! the actual window silhouette stays rectangular. Setting a real rounded
//! `HRGN` via `SetWindowRgn` shapes the OS window itself, which is what
//! DWM actually composites. As a side effect this also suppresses the
//! thin accent-colored resize border Windows draws around ordinary
//! `WS_THICKFRAME` windows, since that border belongs to the standard
//! rectangular non-client frame this custom region replaces.

#[cfg(target_os = "windows")]
pub const CORNER_RADIUS_DIP: i32 = 10;

/// Handles `WM_NCCALCSIZE` by leaving the proposed rect untouched, which
/// makes the client area equal the full window rect. Without this, Windows
/// still reserves a few invisible pixels of resize border around a
/// WS_THICKFRAME window even with decorations off — `outer_size()` (window
/// rect) and `inner_size()` (client rect) end up different, and that gap
/// showed up as an unpainted white strip (the native window background)
/// plus titlebar buttons that were positioned relative to the wrong edge.
/// This app already does its own resize hit-testing (ResizeBorders.svelte),
/// so nothing depends on that native margin existing.
#[cfg(target_os = "windows")]
unsafe extern "system" fn subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _id_subclass: usize,
    _ref_data: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::Shell::DefSubclassProc;
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsZoomed, NCCALCSIZE_PARAMS, WM_NCCALCSIZE};

    if msg == WM_NCCALCSIZE && wparam != 0 {
        if IsZoomed(hwnd) != 0 {
            // A maximized WS_THICKFRAME window is normally sized larger
            // than the monitor's work area to compensate for the standard
            // resize-frame border, which Windows then subtracts back out
            // when it computes the client rect — the two cancel out so
            // content lands flush with the work area. Since we skip that
            // subtraction below (for the restored-window case, to get
            // edge-to-edge content with no invisible margin), a maximized
            // window needs an explicit fix here too, or its content drifts
            // off the work area. A GetSystemMetrics-based inset assumes
            // Windows expanded the proposed rect symmetrically on all four
            // sides, which isn't reliably true (it left a left/top gap
            // here) — setting the rect directly to the monitor's work area
            // is exact regardless of that assumption.
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) != 0 {
                let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                params.rgrc[0] = info.rcWork;
            }
        }
        return 0;
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(target_os = "windows")]
pub fn remove_nonclient_frame(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;

    let Ok(hwnd) = window.hwnd() else { return };
    let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
    unsafe {
        SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn remove_nonclient_frame(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "windows")]
pub fn apply(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        ClientToScreen, CreateRectRgn, CreateRoundRectRgn, SetWindowRgn,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect};

    let Ok(hwnd) = window.hwnd() else { return };
    let Ok(maximized) = window.is_maximized() else { return };
    let scale = window.scale_factor().unwrap_or(1.0);
    let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;

    unsafe {
        let mut window_rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut window_rect) == 0 {
            return;
        }
        let width = window_rect.right - window_rect.left;
        let height = window_rect.bottom - window_rect.top;
        if width <= 0 || height <= 0 {
            return;
        }

        let region = if maximized {
            // Even with our WM_NCCALCSIZE override forcing the client
            // rect to exactly the monitor's work area, Windows still
            // internally sizes/positions a maximized WS_THICKFRAME
            // window's OUTER rect a few pixels larger than that (its own
            // legacy frame-border math, decided before WM_NCCALCSIZE even
            // runs, so unaffected by it). Assuming the client sits flush
            // at the window's local (0,0) left that gap exposed as an
            // unpainted native-background margin, worse on top/left than
            // bottom/right. Query the client's real on-screen position
            // and clip to exactly that instead of assuming it's flush.
            let mut client_rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client_rect);
            let mut origin = POINT { x: 0, y: 0 };
            ClientToScreen(hwnd, &mut origin);
            let offset_x = origin.x - window_rect.left;
            let offset_y = origin.y - window_rect.top;
            CreateRectRgn(
                offset_x,
                offset_y,
                offset_x + (client_rect.right - client_rect.left),
                offset_y + (client_rect.bottom - client_rect.top),
            )
        } else {
            let radius = ((CORNER_RADIUS_DIP as f64) * scale).round() as i32;
            // CreateRoundRectRgn treats right/bottom as exclusive, so the
            // +1 keeps the region covering the window's full outer rect.
            CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2)
        };
        SetWindowRgn(hwnd, region, 1);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply(_window: &tauri::WebviewWindow) {}
