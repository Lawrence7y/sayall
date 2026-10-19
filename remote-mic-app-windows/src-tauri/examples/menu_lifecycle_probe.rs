//! Native lifecycle regression: no BLE, mappings or third-party app changes.
#[path = "../src/quick_menu_overlay.rs"]
mod quick_menu_overlay;
use std::{
    collections::BTreeMap,
    sync::{mpsc, Arc},
    time::Duration,
};
use tauri::Manager;

fn native_state(window: &tauri::WebviewWindow) -> Option<(bool, bool)> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GetWindowLongPtrW, IsWindowVisible, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
        },
    };
    let hwnd = HWND(window.hwnd().ok()?.0);
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    Some((
        unsafe { IsWindowVisible(hwnd).as_bool() },
        style & WS_EX_TOOLWINDOW.0 != 0 && style & WS_EX_APPWINDOW.0 == 0,
    ))
}

fn main() {
    if !std::env::args().any(|arg| arg == "--exercise") {
        std::process::exit(2);
    }
    tauri::Builder::default()
        .setup(|app| {
            let (ready_tx, ready_rx) = mpsc::channel();
            quick_menu_overlay::register(
                app.handle(),
                Arc::new(move |id| {
                    let _ = ready_tx.send(id);
                }),
                Arc::new(|_| sayall_windows::quick_menu::publish(None)),
            );
            // The production startup also creates the main window after registration.
            let main = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("SayAll lifecycle probe")
            .visible(false)
            .focused(false)
            .build()?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(800));
                let absent = handle
                    .get_webview_window(quick_menu_overlay::WINDOW_LABEL)
                    .is_none();
                println!("startup menu_absent={absent}");
                let mut passed = absent;
                let mut original = None;
                for id in [810001, 810002] {
                    sayall_windows::quick_menu::publish(Some(
                        sayall_windows::quick_menu::Presentation {
                            generation: id,
                            labels: BTreeMap::new(),
                        },
                    ));
                    let ready = ready_rx.recv_timeout(Duration::from_secs(6)).ok() == Some(id);
                    let window = handle
                        .get_webview_window(quick_menu_overlay::WINDOW_LABEL)
                        .unwrap();
                    let state = native_state(&window);
                    let hwnd = window.hwnd().unwrap().0 as usize;
                    let reused = original.is_none() || original == Some(hwnd);
                    original = Some(hwnd);
                    println!(
                        "open generation={id} ready={ready} native_state={state:?} reused={reused}"
                    );
                    passed &= ready && state == Some((true, true)) && reused;
                    // Simulate the observed divergence: framework cached-hidden, but
                    // Win32 visible/app-window. No physical input is synthesized.
                    let (tx, rx) = mpsc::channel();
                    let ui = window.clone();
                    handle
                        .run_on_main_thread(move || {
                            use windows::Win32::{
                                Foundation::HWND,
                                UI::WindowsAndMessaging::{
                                    GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE,
                                    SW_SHOWNOACTIVATE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
                                },
                            };
                            let _ = ui.hide();
                            let native = HWND(ui.hwnd().unwrap().0);
                            unsafe {
                                let old = GetWindowLongPtrW(native, GWL_EXSTYLE) as u32;
                                SetWindowLongPtrW(
                                    native,
                                    GWL_EXSTYLE,
                                    ((old | WS_EX_APPWINDOW.0) & !WS_EX_TOOLWINDOW.0) as isize,
                                );
                                let _ = ShowWindow(native, SW_SHOWNOACTIVATE);
                            }
                            let _ = tx.send((ui.is_visible().unwrap_or(true), native_state(&ui)));
                        })
                        .unwrap();
                    let drift = rx.recv_timeout(Duration::from_secs(3)).unwrap();
                    println!(
                        "drift generation={id} framework_visible={} native_state={:?}",
                        drift.0, drift.1
                    );
                    passed &= drift.1 == Some((true, false));
                    if id == 810002 {
                        use windows::Win32::{
                            Foundation::HWND,
                            UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE},
                        };
                        unsafe {
                            PostMessageW(
                                Some(HWND(window.hwnd().unwrap().0)),
                                WM_CLOSE,
                                Default::default(),
                                Default::default(),
                            )
                            .unwrap();
                        }
                        println!("native_close generation={id} requested=true");
                    } else {
                        sayall_windows::quick_menu::publish(None);
                    }
                    let until = std::time::Instant::now() + Duration::from_secs(2);
                    let hidden = loop {
                        let hidden = native_state(&window) == Some((false, true));
                        if hidden || std::time::Instant::now() >= until {
                            break hidden;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    };
                    println!(
                        "cancel generation={id} native_hidden_tool={hidden} native_state={:?}",
                        native_state(&window)
                    );
                    passed &= hidden;
                }
                sayall_windows::quick_menu::publish(Some(
                    sayall_windows::quick_menu::Presentation {
                        generation: 810003,
                        labels: BTreeMap::new(),
                    },
                ));
                sayall_windows::quick_menu::publish(None);
                std::thread::sleep(Duration::from_millis(500));
                let stale_hidden = handle
                    .get_webview_window(quick_menu_overlay::WINDOW_LABEL)
                    .is_some_and(|w| native_state(&w) == Some((false, true)));
                println!("cancel_before_load native_hidden_tool={stale_hidden}");
                let _ = main;
                handle.exit(if passed && stale_hidden { 0 } else { 1 });
            });
            let guard = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(20));
                guard.exit(1);
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("lifecycle probe build")
        .run(|_, event| match event {
            tauri::RunEvent::ExitRequested {
                api, code: None, ..
            } => api.prevent_exit(),
            tauri::RunEvent::ExitRequested { .. } => sayall_windows::quick_menu::set_listener(None),
            _ => {}
        });
}
