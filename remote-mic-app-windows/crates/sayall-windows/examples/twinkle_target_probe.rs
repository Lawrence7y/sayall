//! Explicit single-monitor hardware verification; an independent read-only probe is required.
#[cfg(windows)]
fn main() -> Result<(), String> {
    use sayall_windows::{send_input::ScrollDirection, twinkle_tray as tray};
    use std::{os::windows::process::CommandExt, process::Command, sync::mpsc, time::Duration};
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 || args[1] != "--exercise" {
        return Err("requires --exercise <independent-readback-exe>".into());
    }
    let read = || -> Result<u32, String> {
        let output = Command::new(&args[2])
            .creation_flags(0x08000000)
            .output()
            .map_err(|_| "readback spawn")?;
        if !output.status.success() {
            return Err("readback failed".into());
        }
        let text = String::from_utf8(output.stdout).map_err(|_| "readback UTF8")?;
        if !text.contains("hardware readable_count=1") || text.matches("type=ddcci").count() != 1 {
            return Err("requires exactly one readable and controllable monitor".into());
        }
        text.lines()
            .find_map(|l| {
                l.strip_prefix("hardware brightness_raw=")
                    .and_then(|s| s.strip_suffix(" max=100"))
                    .and_then(|s| s.parse().ok())
            })
            .ok_or("unsupported readback scale".into())
    };
    fn wait(call: impl FnOnce(tray::Completion) -> Result<(), String>) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        call(Box::new(move |r| {
            let _ = tx.send(r);
        }))?;
        rx.recv_timeout(Duration::from_secs(60))
            .map_err(|_| "completion timeout")?
    }
    let baseline = read()?;
    if !(5..=94).contains(&baseline) {
        return Err("brightness near boundary".into());
    }
    let result = (|| {
        assert!(tray::target_snapshot().is_all);
        wait(tray::request_cycle)?;
        if tray::target_snapshot().is_all {
            return Err("cycle did not select single display".into());
        }
        if read()? != baseline {
            return Err("cycle unexpectedly changed brightness".into());
        }
        println!("cycle target=single brightness_unchanged=true");
        wait(|c| tray::request_selected(ScrollDirection::Up, 1, c))?;
        std::thread::sleep(Duration::from_millis(500));
        if read()? != baseline + 1 {
            return Err("selected adjustment did not reach hardware".into());
        }
        println!("selected_adjust hardware_confirmed=true");
        wait(tray::request_cycle)?;
        if !tray::target_snapshot().is_all || read()? != baseline + 1 {
            return Err("cycle back failed or changed brightness".into());
        }
        println!("cycle target=all brightness_unchanged=true");
        wait(|c| tray::request_selected(ScrollDirection::Down, 1, c))?;
        std::thread::sleep(Duration::from_millis(500));
        if read()? != baseline {
            return Err("selected all adjustment mismatch".into());
        }
        // Queue cycle and adjust together: adjustment must resolve the target at execution.
        let (tx, rx) = mpsc::channel();
        for (cycle, direction) in [
            (true, ScrollDirection::Up),
            (false, ScrollDirection::Up),
            (true, ScrollDirection::Down),
            (false, ScrollDirection::Down),
        ] {
            let tx = tx.clone();
            let completion: tray::Completion = Box::new(move |r| {
                let _ = tx.send(r);
            });
            if cycle {
                tray::request_cycle(completion)?;
            } else {
                tray::request_selected(direction, 1, completion)?;
            }
        }
        for _ in 0..4 {
            rx.recv_timeout(Duration::from_secs(60))
                .map_err(|_| "FIFO timeout")??;
        }
        std::thread::sleep(Duration::from_millis(500));
        if !tray::target_snapshot().is_all || read()? != baseline {
            return Err("FIFO target/adjustment mismatch".into());
        }
        println!("queued_cycle_adjust result=passed final_target=all final_brightness={baseline}");
        Ok(())
    })();
    // Restore even if an assertion above failed. Safe only because read() gates on one monitor.
    let exe = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("no localappdata")?)
        .join("Programs/twinkle-tray/Twinkle Tray.exe");
    let restore = Command::new(exe)
        .args(["--All".to_owned(), format!("--Set={baseline}")])
        .creation_flags(0x08000000)
        .status()
        .map_err(|_| "restore spawn failed");
    std::thread::sleep(Duration::from_secs(1));
    let restored = restore.is_ok_and(|s| s.success()) && read().is_ok_and(|v| v == baseline);
    tray::shutdown();
    println!("restore hardware_confirmed={restored} baseline={baseline}");
    if !restored {
        return Err("RESTORATION FAILED: check screen brightness".into());
    }
    result
}
#[cfg(not(windows))]
fn main() {
    eprintln!("requires Windows");
    std::process::exit(1);
}
