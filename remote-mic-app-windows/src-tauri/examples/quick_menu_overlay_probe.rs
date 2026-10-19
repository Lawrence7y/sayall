//! Exercise the production cached menu window; no mappings or hardware change.
#[path = "../src/quick_menu_overlay.rs"]
mod quick_menu_overlay;
use std::{
    collections::BTreeMap,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tauri::Manager;

fn main() {
    if !std::env::args().any(|argument| argument == "--exercise") {
        std::process::exit(2);
    }
    tauri::Builder::default()
        .setup(|app| {
            let foreground = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0 as usize;
            let (ready_tx, ready_rx) = mpsc::channel();
            let (cancel_tx, cancel_rx) = mpsc::channel();
            quick_menu_overlay::register(app.handle(), Arc::new(move |generation| { let _ = ready_tx.send(generation); }), Arc::new(move |generation| {
                let _ = cancel_tx.send(generation);
                sayall_windows::quick_menu::publish(None);
            }));
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                // First request races renderer startup intentionally (cold path).
                use sayall_windows::raw_input::RemoteButton::*;
                let mut original_hwnd = None;
                let mut passed = true;
                let apps = vec![
                    sayall_windows::app_launcher::CustomAppPick { name: "DeepSeek Harness".into(), path: r"shell:AppsFolder\Fixture.Harness!App".into() },
                    sayall_windows::app_launcher::CustomAppPick { name: "ChatGPT".into(), path: r"shell:AppsFolder\Fixture.ChatGPT!App".into() },
                    sayall_windows::app_launcher::CustomAppPick { name: "性能监视器".into(), path: r"C:\Fixture\PERFMON.EXE".into() },
                ];
                for generation in 900001..=900003 {
                    let start = Instant::now();
                    let mut menu = sayall_windows::quick_menu::QuickMenu::default();
                    menu.open(BTreeMap::from([
                        (Up, sayall_windows::send_input::ButtonAction::TwinkleCycleMonitor),
                        (Right, sayall_windows::send_input::ButtonAction::OpenApp { target: apps[0].path.clone() }),
                        (Left, sayall_windows::send_input::ButtonAction::OpenApp { target: apps[1].path.clone() }),
                        (Down, sayall_windows::send_input::ButtonAction::OpenApp { target: apps[2].path.clone() }),
                    ]), Instant::now());
                    let mut presentation = menu.presentation_with_apps(&apps).unwrap();
                    assert_eq!(presentation.labels[&Right], "DSH");
                    assert_eq!(presentation.labels[&Left], "ChatGPT");
                    assert_eq!(presentation.labels[&Down], "性能监视");
                    presentation.generation = generation;
                    // Only this probe adds a short generation marker for UIA.
                    presentation.labels.insert(Up, format!("切屏 {generation}"));
                    sayall_windows::quick_menu::publish(Some(presentation));
                    let ready = ready_rx.recv_timeout(Duration::from_secs(6)).ok() == Some(generation);
                    let elapsed_ms = start.elapsed().as_millis();
                    let window = handle.get_webview_window(quick_menu_overlay::WINDOW_LABEL);
                    let visible = window.as_ref().is_some_and(|window| window.is_visible().unwrap_or(false));
                    let hwnd = window.as_ref().and_then(|window| window.hwnd().ok()).map(|hwnd| hwnd.0 as usize);
                    let reused = original_hwnd.is_none() || original_hwnd == hwnd;
                    original_hwnd = hwnd;
                    let preserved = foreground != 0 && unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0 as usize == foreground;
                    let anchored = window.as_ref().is_some_and(|window| {
                        let std::result::Result::Ok(Some(monitor)) = window.primary_monitor() else { return false; };
                        let std::result::Result::Ok(position) = window.outer_position() else { return false; };
                        let std::result::Result::Ok(size) = window.inner_size() else { return false; };
                        let area = monitor.work_area();
                        let scale = monitor.scale_factor();
                        let expected_x = f64::from(area.position.x) + f64::from(area.size.width) - 244.0 * scale;
                        let expected_y = f64::from(area.position.y) + f64::from(area.size.height) - 256.0 * scale;
                        (f64::from(position.x) - expected_x).abs() <= 2.0 && (f64::from(position.y) - expected_y).abs() <= 2.0
                            && (f64::from(size.width) / scale - 232.0).abs() < 2.0
                            && (f64::from(size.height) / scale - 244.0).abs() < 2.0
                    });
                    println!("menu generation={generation} ready={ready} visible={visible} focus_preserved={preserved} reused={reused} anchored_above_clock={anchored} elapsed_ms={elapsed_ms}");
                    passed &= anchored;
                    if generation == 900001 {
                        if let Some(window) = &window {
                            let _ = window.eval("{const a=document.querySelector('.ring').getAnimations()[0];if(a){a.pause();a.currentTime=80;}}");
                            println!("ring_frame=early");
                            std::thread::sleep(Duration::from_millis(1500));
                            let _ = window.eval("{const a=document.querySelector('.ring').getAnimations()[0];if(a){a.currentTime=480;}}");
                            println!("ring_frame=late");
                            std::thread::sleep(Duration::from_millis(1500));
                        }
                    }
                    if generation == 900003 {
                        let cancelled = cancel_rx.recv_timeout(Duration::from_secs(5)).ok() == Some(generation);
                        let focus_preserved = foreground != 0 && unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0 as usize == foreground;
                        println!("menu mouse_cancel_received={cancelled} click_focus_preserved={focus_preserved}");
                        passed &= cancelled && focus_preserved;
                    } else {
                        std::thread::sleep(Duration::from_millis(1500));
                        sayall_windows::quick_menu::publish(None);
                    }
                    let deadline = Instant::now() + Duration::from_secs(3);
                    let hidden = loop {
                        let hidden = handle.get_webview_window(quick_menu_overlay::WINDOW_LABEL).is_some_and(|window| !window.is_visible().unwrap_or(true));
                        if hidden || Instant::now() >= deadline { break hidden; }
                        std::thread::sleep(Duration::from_millis(10));
                    };
                    println!("menu generation={generation} hidden={hidden}");
                    passed &= ready && visible && preserved && reused && hidden;
                    std::thread::sleep(Duration::from_millis(200));
                }
                // Cancellation before a load completes must not resurrect a menu.
                sayall_windows::quick_menu::publish(Some(sayall_windows::quick_menu::Presentation { generation: 900004, labels: BTreeMap::new() }));
                sayall_windows::quick_menu::publish(None);
                std::thread::sleep(Duration::from_millis(400));
                let stale_hidden = handle.get_webview_window(quick_menu_overlay::WINDOW_LABEL).is_some_and(|window| !window.is_visible().unwrap_or(true));
                println!("menu cancelled_load_hidden={stale_hidden}");
                handle.exit(if passed && stale_hidden { 0 } else { 1 });
            });
            let guard = app.handle().clone();
            std::thread::spawn(move || { std::thread::sleep(Duration::from_secs(25)); guard.exit(1); });
            Ok(())
        })
        .build(tauri::generate_context!()).expect("menu probe build")
        .run(|_, event| match event {
            tauri::RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            tauri::RunEvent::ExitRequested { .. } => sayall_windows::quick_menu::set_listener(None),
            _ => {}
        });
}
