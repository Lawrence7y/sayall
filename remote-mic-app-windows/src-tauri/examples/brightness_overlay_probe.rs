//! Verify the production overlay without changing mappings or brightness.
//! Build with --release --features tauri/custom-protocol; embed a Common-Controls v6
//! manifest for this standalone example (the normal Tauri bundle already includes one).
#[path = "../src/brightness_overlay.rs"]
mod brightness_overlay;
use tauri::Manager;

fn main() {
    if !std::env::args().any(|a| a == "--exercise") {
        eprintln!("requires --exercise; briefly shows the production target overlay");
        std::process::exit(1);
    }
    tauri::Builder::default()
        .setup(|app| {
            brightness_overlay::register(app.handle());
            let foreground =
                unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0
                    as usize;
            let handle = app.handle().clone();
            sayall_windows::twinkle_tray::request_cycle(Box::new(move |result| {
                if result.is_err() {
                    eprintln!("target cycle failed");
                    handle.exit(1);
                    return;
                }
                std::thread::spawn(move || {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    let visible = loop {
                        let visible = handle.webview_windows().values().any(|w| {
                            w.label().starts_with("brightness-target-")
                                && w.is_visible().unwrap_or(false)
                        });
                        if visible || std::time::Instant::now() >= deadline {
                            break visible;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    };
                    let preserved = foreground != 0
                        && unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }
                            .0 as usize
                            == foreground;
                    println!("overlay visible={visible} focus_preserved={preserved}");
                    if !visible || !preserved {
                        handle.exit(1);
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    let closed = !handle
                        .webview_windows()
                        .values()
                        .any(|w| w.label().starts_with("brightness-target-"));
                    println!("overlay destroyed={closed}");
                    handle.exit(if closed { 0 } else { 1 });
                });
            }))?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(15));
                handle.exit(1);
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("build overlay probe")
        .run(|_, event| match event {
            tauri::RunEvent::ExitRequested {
                api, code: None, ..
            } => api.prevent_exit(),
            tauri::RunEvent::ExitRequested { .. } => sayall_windows::twinkle_tray::shutdown(),
            _ => {}
        });
}
