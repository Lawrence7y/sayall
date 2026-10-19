//! Short-lived, non-focusable display-target feedback. No third-party state is read here.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use tauri::Manager;
static GENERATION: AtomicU64 = AtomicU64::new(1);
static LAST_WINDOW: Mutex<Option<String>> = Mutex::new(None);

pub fn register(app: &tauri::AppHandle) {
    let app = app.clone();
    sayall_windows::twinkle_tray::set_target_listener(Some(Arc::new(move |snapshot| {
        let handle = app.clone();
        let ui = handle.clone();
        if handle
            .run_on_main_thread(move || show(&ui, &snapshot.name))
            .is_err()
        {
            sayall_windows::gatt_note(
                "twinkle_overlay phase=completed result=failed reason=ui_dispatch".into(),
            );
        }
    })));
}

fn close_later(app: tauri::AppHandle, label: String, milliseconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(milliseconds));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = handle.get_webview_window(&label) {
                let _ = window.destroy();
                sayall_windows::gatt_note("twinkle_overlay phase=closed reason=timeout".into());
            }
            let mut last = LAST_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
            if last.as_deref() == Some(&label) {
                *last = None;
            }
        });
    });
}

fn show(app: &tauri::AppHandle, name: &str) {
    let label = format!(
        "brightness-target-{}",
        GENERATION.fetch_add(1, Ordering::Relaxed)
    );
    let old = LAST_WINDOW
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(label.clone());
    if let Some(old) = old {
        if let Some(window) = app.get_webview_window(&old) {
            let _ = window.destroy();
        }
    }
    // JSON quoting plus textContent: monitor names cannot become HTML or script.
    let encoded = serde_json::to_string(name).unwrap_or_else(|_| "\"显示器\"".into());
    let script = format!("document.addEventListener('DOMContentLoaded',()=>{{document.getElementById('target').textContent={encoded};}});");
    let shown = Arc::new(AtomicBool::new(false));
    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App("brightness-target.html".into()),
    )
    .title("亮度控制目标")
    .inner_size(420.0, 86.0)
    .decorations(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .visible(false)
    .focused(false)
    .focusable(false)
    .background_color(tauri::webview::Color(32, 35, 41, 255))
    .initialization_script(script)
    .on_page_load(move |window, payload| {
        if payload.event() != tauri::webview::PageLoadEvent::Finished
            || !payload.url().path().ends_with("/brightness-target.html")
        {
            return;
        }
        let current = LAST_WINDOW
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            == Some(window.label());
        if !current || shown.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = window.set_ignore_cursor_events(true);
        if window.show().is_err() {
            sayall_windows::gatt_note(
                "twinkle_overlay phase=completed result=failed reason=window_show".into(),
            );
            return;
        }
        sayall_windows::gatt_note(
            "twinkle_overlay phase=shown result=passed focusable=false".into(),
        );
        // Count visible time after loading, not controller initialization.
        close_later(window.app_handle().clone(), window.label().to_owned(), 1800);
    });
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        builder = builder.position(
            (f64::from(monitor.position().x) + f64::from(monitor.size().width) / 2.0) / scale
                - 210.0,
            f64::from(monitor.position().y) / scale + 60.0,
        );
    }
    if builder.build().is_err() {
        let mut last = LAST_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() == Some(&label) {
            *last = None;
        }
        sayall_windows::gatt_note(
            "twinkle_overlay phase=completed result=failed reason=window_create".into(),
        );
        return;
    }
    // Also clean up a hidden window if its page never finishes loading.
    close_later(app.clone(), label, 8000);
}
