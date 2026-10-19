//! Non-activating remote menu, created on first use and then reused hidden.
//! Native visibility/style readback prevents inactive ghost/taskbar windows.
//! The engine still owns selection, cancellation and timeout.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Instant;
use tauri::Manager;

pub const WINDOW_LABEL: &str = "quick-menu";
const MENU_WIDTH: f64 = 232.0;
const MENU_HEIGHT: f64 = 244.0;

// Work area excludes the taskbar: anchor just above its clock, not over it.
fn clock_anchor(x: i32, y: i32, width: u32, height: u32, scale: f64) -> (f64, f64) {
    let left = f64::from(x) / scale;
    let top = f64::from(y) / scale;
    (
        (left + f64::from(width) / scale - MENU_WIDTH - 12.0).max(left),
        (top + f64::from(height) / scale - MENU_HEIGHT - 12.0).max(top),
    )
}

fn monitor_anchor(monitor: &tauri::Monitor) -> (f64, f64) {
    let area = monitor.work_area();
    clock_anchor(
        area.position.x,
        area.position.y,
        area.size.width,
        area.size.height,
        monitor.scale_factor(),
    )
}
static ACTIVE: Mutex<Option<(u64, Instant)>> = Mutex::new(None);
static LOADED: AtomicBool = AtomicBool::new(false);
static SHOWN: AtomicU64 = AtomicU64::new(0);
static PENDING: Mutex<Option<sayall_windows::quick_menu::Presentation>> = Mutex::new(None);
type Signal = Arc<dyn Fn(u64) + Send + Sync>;
#[derive(Clone)]
struct Signals {
    ready: Signal,
    cancel: Signal,
}

pub fn register(app: &tauri::AppHandle, ready: Signal, cancel: Signal) {
    let signals = Signals { ready, cancel };
    // Do not create a menu/renderer during startup. Create it on first request,
    // then reuse it hidden. This also avoids a startup taskbar/ghost window.
    let handle = app.clone();
    sayall_windows::quick_menu::set_listener(Some(Arc::new(move |presentation| {
        let app = handle.clone();
        let ui = app.clone();
        let generation = presentation.as_ref().map(|item| item.generation);
        let failed = signals.cancel.clone();
        let signals = signals.clone();
        if app
            .run_on_main_thread(move || update(&ui, presentation, signals))
            .is_err()
        {
            if let Some(generation) = generation {
                failed(generation);
            }
            sayall_windows::gatt_note("quick_menu_overlay phase=failed reason=ui_dispatch".into());
        }
    })));
}

fn page_generation(url: &tauri::Url) -> Option<u64> {
    if !url.path().ends_with("/quick-menu.html") {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "generation")?
        .1
        .parse()
        .ok()
}

fn cancel_generation(url: &tauri::Url) -> Option<u64> {
    if url.scheme() != "https"
        || url.host_str() != Some("sayall.invalid")
        || url.path() != "/menu-cancel"
    {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "generation")?
        .1
        .parse()
        .ok()
        .filter(|id| *id != 0)
}

// Preserve unrelated flags, but never allow this overlay to become an app window.
#[cfg(windows)]
fn menu_ex_style(style: u32) -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    (style & !WS_EX_APPWINDOW.0) | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0
}

/// Run on the UI thread. Framework hide/show can be a no-op when its cached
/// visibility diverges from Win32. ShowWindow returns the *previous* state, so
/// only IsWindowVisible + style readback count as success here.
fn set_visible_verified(window: &tauri::WebviewWindow, visible: bool) -> tauri::Result<()> {
    let framework = if visible {
        window.show()
    } else {
        window.hide()
    };
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{
                GetWindowLongPtrW, IsWindowVisible, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE,
                SW_HIDE, SW_SHOWNOACTIVATE,
            },
        };
        let hwnd = HWND(window.hwnd()?.0);
        unsafe {
            let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
            let old = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let expected = menu_ex_style(old);
            if old != expected {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, expected as isize);
            }
            let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if IsWindowVisible(hwnd).as_bool() == visible && actual == menu_ex_style(actual) {
                return Ok(());
            }
        }
        let _ = framework;
        Err(std::io::Error::other("menu native visibility/style readback failed").into())
    }
    #[cfg(not(windows))]
    {
        framework?;
        if window.is_visible()? == visible {
            Ok(())
        } else {
            Err(std::io::Error::other("menu visibility readback failed").into())
        }
    }
}

fn hide_or_destroy(window: &tauri::WebviewWindow, reason: &str) {
    // Reset document opacity too, so an external OS show cannot expose an old
    // ring while the engine has no presentation. No navigation or new renderer.
    let _ = window.eval("document.documentElement.classList.remove('is-visible');");
    let hidden = set_visible_verified(window, false).is_ok();
    let destroyed = if hidden {
        false
    } else {
        LOADED.store(false, Ordering::Release);
        window.destroy().is_ok()
    };
    sayall_windows::gatt_note(format!(
        "quick_menu_overlay phase=hidden reason={reason} native_verified={hidden} destroy_requested={destroyed} terminal_result={}",
        if hidden { "passed" } else if destroyed { "accepted" } else { "failed" }
    ));
}

fn hide_if_inactive(window: &tauri::WebviewWindow, reason: &str) {
    if ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
        hide_or_destroy(window, reason);
    }
}

fn ensure_window(app: &tauri::AppHandle, signals: Signals) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        return Ok(window);
    }
    LOADED.store(false, Ordering::Release);
    let mouse_cancel = signals.cancel.clone();
    let close_cancel = signals.cancel.clone();
    let navigation_app = app.clone();
    let mut builder = tauri::WebviewWindowBuilder::new(
        app, WINDOW_LABEL, tauri::WebviewUrl::App("quick-menu.html".into()),
    )
    .title("四向快捷菜单")
    .inner_size(MENU_WIDTH, MENU_HEIGHT)
    .transparent(true).shadow(false)
    .decorations(false).resizable(false).skip_taskbar(true)
    .always_on_top(true).visible(false).focused(false).focusable(false)
    .background_color(tauri::webview::Color(0, 0, 0, 0))
    .initialization_script("document.addEventListener('DOMContentLoaded',()=>{try{const labels=JSON.parse(new URLSearchParams(location.search).get('labels')||'{}');for(const direction of ['up','down','left','right']){const element=document.getElementById(direction);element.textContent=labels[direction]||'未设置';element.title=element.textContent;}}catch{}});")
    .on_navigation(move |url| {
        if url.host_str() == Some("sayall.invalid") {
            if let Some(generation) = cancel_generation(url) {
                let active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|(id, _)| *id) == Some(generation);
                let accepted = active && sayall_windows::quick_menu::is_current_presentation(generation);
                sayall_windows::gatt_note(format!("quick_menu_overlay generation={generation} phase=cancel_requested source=mouse accepted={accepted}"));
                if accepted { mouse_cancel(generation); }
                else {
                    let ui = navigation_app.clone();
                    let _ = navigation_app.run_on_main_thread(move || {
                        if let Some(window) = ui.get_webview_window(WINDOW_LABEL) {
                            hide_if_inactive(&window, "orphan_mouse_cancel");
                        }
                    });
                }
            }
            // Always block this reserved host, including stale/malformed requests.
            return false;
        }
        true
    })
    .on_page_load(move |window, payload| {
        sayall_windows::gatt_note(format!("quick_menu_overlay phase=page_load finished={} menu_document={} has_generation={}", payload.event() == tauri::webview::PageLoadEvent::Finished, payload.url().path().ends_with("/quick-menu.html"), page_generation(payload.url()).is_some()));
        if payload.event() != tauri::webview::PageLoadEvent::Finished { return; }
        let Some(generation) = page_generation(payload.url()) else {
            if payload.url().path().ends_with("/quick-menu.html") {
                LOADED.store(true, Ordering::Release);
                hide_if_inactive(&window, "initial_document");
                let pending = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(presentation) = pending {
                    update(window.app_handle(), Some(presentation), signals.clone());
                }
            }
            return;
        };
        let started = {
            let active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
            match *active {
                Some((id, started)) if id == generation => started,
                _ => {
                    drop(active);
                    hide_if_inactive(&window, "stale_document");
                    return;
                }
            }
        };
        // Both engine and window generation must match: an old navigation must
        // never show a cancelled menu or mark a newer presentation as ready.
        if !sayall_windows::quick_menu::is_current_presentation(generation)
            || window.url().ok().as_ref().and_then(page_generation) != Some(generation) {
            return;
        }
        // WebView can deliver Finished again for the same document. Never
        // re-show an already presented generation or emit duplicate ready ACKs.
        let first_show = SHOWN.load(Ordering::Acquire) != generation;
        if (first_show && (window.set_ignore_cursor_events(false).is_err() || set_visible_verified(&window, true).is_err()))
            || window.eval("document.documentElement.classList.add('is-visible');").is_err() {
            *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            hide_or_destroy(&window, "show_failed");
            (signals.cancel)(generation);
            sayall_windows::gatt_note("quick_menu_overlay phase=failed reason=window_show".into());
            return;
        }
        if !first_show { return; }
        SHOWN.store(generation, Ordering::Release);
        (signals.ready)(generation);
        sayall_windows::gatt_note(format!("quick_menu_overlay generation={generation} phase=shown focusable=false reused=true anchor=clock_above animation_ms=480 elapsed_ms={}", started.elapsed().as_millis()));
    });
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let (x, y) = monitor_anchor(&monitor);
        builder = builder.position(x, y);
    }
    let window = builder.build()?;
    if let Err(error) = set_visible_verified(&window, false) {
        let _ = window.destroy();
        return Err(error);
    }
    let close_app = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let generation = ACTIVE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map(|(id, _)| *id);
            if let Some(generation) = generation {
                close_cancel(generation);
            } else if let Some(menu) = close_app.get_webview_window(WINDOW_LABEL) {
                hide_if_inactive(&menu, "native_close");
            }
        }
    });
    sayall_windows::gatt_note(
        "quick_menu_overlay phase=created_on_demand native_hidden=true".into(),
    );
    Ok(window)
}

fn update(
    app: &tauri::AppHandle,
    presentation: Option<sayall_windows::quick_menu::Presentation>,
    signals: Signals,
) {
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    SHOWN.store(0, Ordering::Release);
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        hide_or_destroy(&window, "presentation_update");
    }
    let Some(presentation) = presentation else {
        return;
    };
    let generation = presentation.generation;
    if !sayall_windows::quick_menu::is_current_presentation(generation) {
        return;
    }
    let started = Instant::now();
    sayall_windows::gatt_note(format!(
        "quick_menu_overlay generation={generation} phase=navigate_requested"
    ));
    let result = (|| -> tauri::Result<()> {
        let window = ensure_window(app, signals.clone())?;
        // Re-anchor every opening so resolution, taskbar and DPI changes do not
        // leave the cached window at an obsolete screen position.
        if let Some(monitor) = app.primary_monitor()? {
            let (x, y) = monitor_anchor(&monitor);
            window.set_size(tauri::LogicalSize::new(MENU_WIDTH, MENU_HEIGHT))?;
            window.set_position(tauri::LogicalPosition::new(x, y))?;
        }
        // A cold WebView can still report about:blank. Wait for the real local
        // document, rather than navigating an about:blank URL with menu data.
        if !LOADED.load(Ordering::Acquire) {
            *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(presentation);
            sayall_windows::gatt_note(format!(
                "quick_menu_overlay generation={generation} phase=waiting_for_preload"
            ));
            return Ok(());
        }
        let mut url = window.url()?;
        url.set_query(None);
        let labels = serde_json::to_string(&presentation.labels).unwrap_or_else(|_| "{}".into());
        url.query_pairs_mut()
            .append_pair("generation", &generation.to_string())
            .append_pair("labels", &labels);
        *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some((generation, started));
        window.navigate(url)
    })();
    if result.is_err() {
        *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        (signals.cancel)(generation);
        sayall_windows::gatt_note("quick_menu_overlay phase=failed reason=window_navigate".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn native_menu_style_removes_appwindow_and_preserves_other_flags() {
        use windows::Win32::UI::WindowsAndMessaging::{
            WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        };
        let original = WS_EX_APPWINDOW.0 | WS_EX_LAYERED.0 | WS_EX_TOPMOST.0;
        let repaired = menu_ex_style(original);
        assert_eq!(repaired & WS_EX_APPWINDOW.0, 0);
        assert_ne!(repaired & WS_EX_TOOLWINDOW.0, 0);
        assert_ne!(repaired & WS_EX_NOACTIVATE.0, 0);
        assert_eq!(
            repaired & (WS_EX_LAYERED.0 | WS_EX_TOPMOST.0),
            original & (WS_EX_LAYERED.0 | WS_EX_TOPMOST.0)
        );
        assert_eq!(menu_ex_style(repaired), repaired);
    }

    #[test]
    fn compact_menu_anchors_above_clock_inside_work_area() {
        assert_eq!(clock_anchor(0, 0, 1920, 1040, 1.0), (1676.0, 784.0));
        assert_eq!(clock_anchor(-1920, 0, 1920, 1040, 1.0), (-244.0, 784.0));
        let (x, y) = clock_anchor(0, 0, 2560, 1392, 1.5);
        assert!((x - 1462.6666667).abs() < 0.001);
        assert_eq!(y, 672.0);
        assert_eq!(clock_anchor(0, 0, 200, 200, 1.0), (0.0, 0.0));
    }

    #[test]
    fn mouse_cancel_url_requires_reserved_host_path_and_valid_generation() {
        assert_eq!(
            cancel_generation(
                &tauri::Url::parse("https://sayall.invalid/menu-cancel?generation=42").unwrap()
            ),
            Some(42)
        );
        for url in [
            "https://example.com/menu-cancel?generation=42",
            "https://sayall.invalid/other?generation=42",
            "https://sayall.invalid/menu-cancel?generation=0",
            "https://sayall.invalid/menu-cancel?generation=bad",
        ] {
            assert_eq!(cancel_generation(&tauri::Url::parse(url).unwrap()), None);
        }
    }

    #[test]
    fn readiness_requires_menu_document_and_valid_generation() {
        assert_eq!(
            page_generation(
                &tauri::Url::parse("tauri://localhost/quick-menu.html?generation=42").unwrap()
            ),
            Some(42)
        );
        for url in [
            "tauri://localhost/quick-menu.html",
            "tauri://localhost/quick-menu.html?generation=bad",
            "tauri://localhost/index.html?generation=42",
        ] {
            assert_eq!(page_generation(&tauri::Url::parse(url).unwrap()), None);
        }
    }
}
