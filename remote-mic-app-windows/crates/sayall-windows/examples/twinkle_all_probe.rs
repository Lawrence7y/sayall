//! Explicit local hardware probe: sends three +1 requests through the production worker.
//! Run only with --exercise; independently read hardware brightness and restore afterwards.
#[cfg(windows)]
fn main() -> Result<(), String> {
    use sayall_windows::{send_input::ScrollDirection, twinkle_tray};
    use std::{sync::mpsc, time::Duration};
    if !std::env::args().any(|arg| arg == "--exercise") {
        return Err("requires --exercise; changes all controllable displays by +3".into());
    }
    struct Shutdown;
    impl Drop for Shutdown {
        fn drop(&mut self) {
            twinkle_tray::shutdown();
        }
    }
    let _shutdown = Shutdown;
    let (sender, receiver) = mpsc::channel();
    for operation in 0..3 {
        let sender = sender.clone();
        twinkle_tray::request(
            ScrollDirection::Up,
            1,
            None,
            Box::new(move |result| {
                let _ = sender.send((operation, result));
            }),
        )?;
    }
    drop(sender);
    for _ in 0..3 {
        let (operation, result) = receiver
            .recv_timeout(Duration::from_secs(20))
            .map_err(|_| "completion timeout")?;
        result?;
        println!(
            "production_worker operation={operation} cli_completed=true hardware_changed=unknown"
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("requires Windows");
    std::process::exit(1);
}
