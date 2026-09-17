use tauri::Manager;

#[tauri::command]
pub fn minimize_window(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn toggle_maximize(window: tauri::Window) -> Result<(), String> {
    let maximized = window.is_maximized().map_err(|err| err.to_string())?;
    if maximized {
        window.unmaximize().map_err(|err| err.to_string())
    } else {
        window.maximize().map_err(|err| err.to_string())
    }
}

#[tauri::command]
pub fn close_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn is_maximized(window: tauri::Window) -> Result<bool, String> {
    window.is_maximized().map_err(|err| err.to_string())
}

/// Ends the app outright. Closing the main window gets there too (lib.rs exits
/// on its Destroyed event, which is what keeps the preloaded, hidden
/// Preferences window from outliving it), but "Quit" shouldn't depend on that
/// chain to mean what it says.
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

const PREFERENCES: &str = "preferences";

/// Set by the Preferences page once its config is applied and painted; reset
/// whenever that page (re)starts loading.
static PREFERENCES_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// An open request that arrived before the page was ready.
static PREFERENCES_SHOW_PENDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

const PRELOAD_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

/// Shows the Preferences window. It is normally preloaded hidden shortly
/// after startup and only hidden on close, so this just reveals it; if the
/// page isn't ready yet, `preferences_ready` shows it as soon as it is.
///
/// Must stay `async`: on Windows, building a webview window from a sync
/// command deadlocks (WebView2), leaving a blank window and a hung app.
#[tauri::command]
pub async fn open_preferences_window(app: tauri::AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering::SeqCst;

    let window = ensure_preferences_window(&app)?;
    PREFERENCES_SHOW_PENDING.store(true, SeqCst);
    if PREFERENCES_READY.load(SeqCst) && PREFERENCES_SHOW_PENDING.swap(false, SeqCst) {
        show_and_focus(&window)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn preferences_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    use std::sync::atomic::Ordering::SeqCst;

    PREFERENCES_READY.store(true, SeqCst);
    if PREFERENCES_SHOW_PENDING.swap(false, SeqCst) {
        show_and_focus(&window)?;
    }
    Ok(())
}

/// Called by each window with its resolved UI language ("es" or "en"), so
/// backend-generated messages and the Preferences window title follow it.
#[tauri::command]
pub async fn set_ui_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    use crate::i18n::{self, Lang, Msg};

    i18n::set(if language == "en" { Lang::En } else { Lang::Es });
    if let Some(window) = app.get_webview_window(PREFERENCES) {
        window
            .set_title(&i18n::tr(Msg::PreferencesTitle))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Registered with `Builder::on_page_load`: preloads Preferences once the
/// main window has loaded (so it doesn't compete with startup), and marks
/// Preferences not ready whenever its page (re)loads.
pub fn on_page_load(webview: &tauri::Webview, payload: &tauri::webview::PageLoadPayload<'_>) {
    use tauri::webview::PageLoadEvent;

    match (webview.label(), payload.event()) {
        (PREFERENCES, PageLoadEvent::Started) => {
            PREFERENCES_READY.store(false, std::sync::atomic::Ordering::SeqCst)
        }
        ("main", PageLoadEvent::Finished) => {
            let app = webview.app_handle().clone();
            // Off the main thread: this callback runs on it, and building a
            // webview window there deadlocks on Windows.
            std::thread::spawn(move || {
                std::thread::sleep(PRELOAD_DELAY);
                let _ = ensure_preferences_window(&app);
            });
        }
        _ => {}
    }
}

/// Windows build 22523+ has DWMWA_SYSTEMBACKDROP_TYPE, which window-vibrancy
/// uses for a smooth Acrylic.
fn windows_has_backdrop_api() -> bool {
    sysinfo::System::kernel_version()
        .and_then(|build| build.trim().parse::<u32>().ok())
        .is_some_and(|build| build >= 22523)
}

fn show_and_focus(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.is_minimized().unwrap_or(false) {
        window.unminimize().map_err(|err| err.to_string())?;
    }
    window.show().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())
}

/// Returns the Preferences window, creating it hidden if needed. This app is
/// a single-page SPA (adapter-static, `ssr = false`), so rather than a second
/// SvelteKit route (which adapter-static's prerendering doesn't cleanly
/// produce as a standalone second HTML entry point), it points at the app
/// root with a `#preferences` fragment; the frontend branches on
/// `location.hash` to decide what to render. Note: the path must be `/`, not
/// `index.html` — in `tauri dev` this resolves against the Vite dev server,
/// which serves `/` but has no literal `/index.html` route (that file only
/// exists post-build), so `index.html#preferences` 404s in dev.
///
/// Must not be called on the main thread (see `open_preferences_window`).
fn ensure_preferences_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    // The preload thread and an open request can race to create it.
    static CREATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = CREATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Some(window) = app.get_webview_window(PREFERENCES) {
        return Ok(window);
    }
    PREFERENCES_READY.store(false, std::sync::atomic::Ordering::SeqCst);

    let builder = tauri::WebviewWindowBuilder::new(
        app,
        PREFERENCES,
        tauri::WebviewUrl::App("/#preferences".into()),
    )
    .title(crate::i18n::tr(crate::i18n::Msg::PreferencesTitle))
    .inner_size(760.0, 480.0)
    .min_inner_size(640.0, 420.0)
    .resizable(true)
    .maximizable(false)
    .transparent(true)
    // It's a settings window for the console behind it: sending it behind that
    // console while you read it is never what you wanted.
    .always_on_top(true)
    // Revealed by show_and_focus once the page is ready; showing it earlier
    // flashes a white, natively framed window and then the default config.
    .visible(false);

    // macOS gets the native window controls drawn over the web title bar,
    // matching the main window's tauri.macos.conf.json (keep the traffic
    // light position in sync with it); elsewhere the web title bar draws
    // its own controls on an undecorated window.
    #[cfg(target_os = "macos")]
    let builder = builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 16.0));
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    let window = builder.build().map_err(|err| err.to_string())?;

    // Window subclassing only works from the thread that owns the HWND, and
    // this runs off the main thread.
    let shaped = window.clone();
    window
        .run_on_main_thread(move || crate::commands::window_shape::attach(&shaped))
        .map_err(|err| err.to_string())?;

    // Keep it alive so the next open is instant.
    let hidden = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hidden.hide();
        }
    });

    Ok(window)
}

/// Native blur behind the window's translucent areas: macOS HUD vibrancy
/// (corner radius matching the window's 10px CSS radius) or Windows Acrylic.
/// Tauri's own `set_effects(None)` never removes vibrancy on macOS (its
/// clear path only exists for Windows), so macOS clears it through
/// window-vibrancy directly — also before applying, so repeated applies
/// don't stack effect views. AppKit requires this on the main thread.
#[tauri::command]
pub fn set_background_blur(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    use tauri::window::{Effect, EffectState, EffectsBuilder};

    let target = window.clone();
    window
        .run_on_main_thread(move || {
            #[cfg(target_os = "macos")]
            let _ = window_vibrancy::clear_vibrancy(&target);
            #[cfg(not(target_os = "macos"))]
            if !enabled {
                let _ = target.set_effects(None::<tauri::utils::config::WindowEffectsConfig>);
            }

            if enabled {
                let effect = if cfg!(target_os = "macos") {
                    Effect::HudWindow
                } else if cfg!(target_os = "windows") && !windows_has_backdrop_api() {
                    // Before build 22523 Acrylic goes through
                    // SetWindowCompositionAttribute, which makes dragging and
                    // resizing the window lag badly; plain blur doesn't.
                    Effect::Blur
                } else {
                    Effect::Acrylic
                };
                let _ = target.set_effects(
                    EffectsBuilder::new()
                        .effect(effect)
                        .state(EffectState::Active)
                        .radius(10.0)
                        .build(),
                );
            }
        })
        .map_err(|err| err.to_string())
}
