//! Twinkle Tray integration uses only its documented command-line interface.
use crate::send_input::ScrollDirection;
use crate::twinkle_displays::{Selection, TargetSnapshot};
use std::sync::Mutex;

static SELECTION: Mutex<Selection> = Mutex::new(Selection::new());
type TargetListener = std::sync::Arc<dyn Fn(TargetSnapshot) + Send + Sync>;
static TARGET_LISTENER: Mutex<Option<TargetListener>> = Mutex::new(None);

pub fn set_target_listener(listener: Option<TargetListener>) {
    *TARGET_LISTENER.lock().unwrap_or_else(|e| e.into_inner()) = listener;
}
fn notify_target() {
    let listener = TARGET_LISTENER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if let Some(listener) = listener {
        listener(target_snapshot());
    }
}

pub fn target_snapshot() -> TargetSnapshot {
    SELECTION
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .snapshot()
}

#[derive(Debug)]
enum Task {
    Arguments(Vec<String>),
    Selected {
        direction: ScrollDirection,
        steps: u16,
    },
    Cycle,
}

pub fn request_selected(
    direction: ScrollDirection,
    steps: u16,
    completion: Completion,
) -> Result<(), String> {
    validate_options(steps, None)?;
    submit(Task::Selected { direction, steps }, completion)
}

pub fn request_cycle(completion: Completion) -> Result<(), String> {
    submit(Task::Cycle, completion)
}

pub type Completion = Box<dyn FnOnce(Result<(), String>) + Send>;

pub fn validate_options(steps: u16, monitor: Option<u16>) -> Result<(), String> {
    if !(1..=100).contains(&steps) {
        return Err("亮度调整步长必须为 1 到 100".into());
    }
    if monitor == Some(0) {
        return Err("Twinkle Tray 显示器编号必须从 1 开始".into());
    }
    Ok(())
}

pub fn arguments(
    direction: ScrollDirection,
    steps: u16,
    monitor: Option<u16>,
) -> Result<Vec<String>, String> {
    validate_options(steps, monitor)?;
    let target = monitor
        .map(|number| format!("--MonitorNum={number}"))
        .unwrap_or_else(|| "--All".into());
    let offset = match direction {
        ScrollDirection::Up => steps as i32,
        ScrollDirection::Down => -(steps as i32),
    };
    Ok(vec![
        target,
        format!("--Offset={offset}"),
        "--Overlay".into(),
    ])
}

pub fn request(
    direction: ScrollDirection,
    steps: u16,
    monitor: Option<u16>,
    completion: Completion,
) -> Result<(), String> {
    submit(brightness_task(direction, steps, monitor)?, completion)
}

fn brightness_task(
    direction: ScrollDirection,
    steps: u16,
    monitor: Option<u16>,
) -> Result<Task, String> {
    validate_options(steps, monitor)?;
    // The previous test build saved monitor=None for both brightness keys.
    // Resolve those keys against the current target in the same FIFO as cycling.
    if monitor.is_none() {
        Ok(Task::Selected { direction, steps })
    } else {
        Ok(Task::Arguments(arguments(direction, steps, monitor)?))
    }
}

fn submit(task: Task, completion: Completion) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::request(task, completion)
    }
    #[cfg(not(windows))]
    {
        let _ = (task, completion);
        Err("亮度控制仅支持 Windows".into())
    }
}

pub fn shutdown() {
    #[cfg(windows)]
    windows_impl::shutdown();
    set_target_listener(None);
}

#[cfg(windows)]
mod windows_impl {
    use super::{Completion, Task, SELECTION};
    use crate::twinkle_displays::{parse_displays, Display, MAX_LIST_BYTES};
    use std::io::Read;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::{self, SyncSender, TrySendError};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::IO::CancelSynchronousIo;

    static STOP: AtomicBool = AtomicBool::new(false);
    static OPERATION: AtomicU64 = AtomicU64::new(1);
    static RUNNER: Mutex<Option<Runner>> = Mutex::new(None);
    struct Job {
        executable: PathBuf,
        task: Task,
        operation: u64,
        completion: Completion,
    }
    struct Runner {
        sender: SyncSender<Job>,
        thread: JoinHandle<()>,
    }

    fn running() -> bool {
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return false;
            };
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut exists = false;
            let mut next = Process32FirstW(snapshot, &mut entry).is_ok();
            while next {
                let length = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
                if name.eq_ignore_ascii_case("Twinkle Tray.exe")
                    || name.eq_ignore_ascii_case("Twinkle-Tray.exe")
                {
                    exists = true;
                    break;
                }
                next = Process32NextW(snapshot, &mut entry).is_ok();
            }
            let _ = CloseHandle(snapshot);
            exists
        }
    }

    fn executable() -> Result<PathBuf, String> {
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or("无法定位 Twinkle Tray")?;
        for relative in [
            "Programs\\twinkle-tray\\Twinkle Tray.exe",
            "Microsoft\\WindowsApps\\Twinkle-Tray.exe",
        ] {
            let path = root.join(relative);
            if path.is_file() {
                return Ok(path);
            }
        }
        Err("未找到 Twinkle Tray，请先安装并启动它".into())
    }

    pub(super) fn request(task: Task, completion: Completion) -> Result<(), String> {
        if STOP.load(Ordering::Acquire) {
            return Err("程序正在退出，亮度请求已取消".into());
        }
        let executable = executable()?;
        if !running() {
            return Err("请先启动 Twinkle Tray，再使用亮度动作".into());
        }
        let mut slot = RUNNER.lock().unwrap_or_else(|error| error.into_inner());
        if STOP.load(Ordering::Acquire) {
            return Err("程序正在退出，亮度请求已取消".into());
        }
        if slot.is_none() {
            let (sender, receiver) = mpsc::sync_channel::<Job>(4);
            let thread = std::thread::Builder::new()
                .name("sayall-brightness".into())
                .spawn(move || {
                    while !STOP.load(Ordering::Acquire) {
                        match receiver.recv_timeout(Duration::from_millis(100)) {
                            Ok(job) => execute(job),
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                while let Ok(job) = receiver.try_recv() {
                    crate::ble::gatt_note(format!("twinkle_brightness operation_id={} phase=completed result=cancelled reason=shutdown_before_execution brightness_changed=unknown", job.operation));
                    (job.completion)(Err("亮度请求已取消".into()));
                    }
                })
                .map_err(|_| "无法启动亮度控制任务")?;
            *slot = Some(Runner { sender, thread });
        }
        let operation = OPERATION.fetch_add(1, Ordering::Relaxed);
        let job = Job {
            executable,
            task,
            operation,
            completion,
        };
        match slot.as_ref().unwrap().sender.try_send(job) {
            Ok(()) => {
                crate::ble::gatt_note(format!(
                    "twinkle_brightness operation_id={operation} phase=queued result=accepted"
                ));
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err("亮度调整请求过多，请稍后再试".into()),
            Err(TrySendError::Disconnected(_)) => Err("亮度控制任务已停止".into()),
        }
    }

    fn execute(job: Job) {
        let started = Instant::now();
        let operation = job.operation;
        let cycle = matches!(&job.task, Task::Cycle);
        let result = (|| {
            if STOP.load(Ordering::Acquire) {
                return Err("亮度请求已取消".into());
            }
            let args = match job.task {
                Task::Arguments(args) => args,
                Task::Cycle => {
                    let displays = list(&job.executable)?;
                    if STOP.load(Ordering::Acquire) {
                        return Err("亮度请求已取消".into());
                    }
                    let mut selection = SELECTION.lock().unwrap_or_else(|e| e.into_inner());
                    selection.cycle(&displays)?;
                    crate::ble::gatt_note(format!("twinkle_target operation_id={operation} controllable_count={} target_all={}", displays.len(), selection.id().is_none()));
                    drop(selection);
                    super::notify_target();
                    return Ok(());
                }
                Task::Selected { direction, steps } => {
                    // Resolve in the same FIFO as cycle jobs, not at enqueue time.
                    let id = SELECTION
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .id()
                        .map(str::to_owned);
                    let mut args = super::arguments(direction, steps, None)?;
                    if let Some(id) = id {
                        let displays = list(&job.executable)?;
                        if !displays.iter().any(|d| d.id == id) {
                            SELECTION.lock().unwrap_or_else(|e| e.into_inner()).reset();
                            super::notify_target();
                            crate::ble::gatt_note(format!("twinkle_target operation_id={operation} reason=target_missing target_all=true brightness_changed=false"));
                            return Err(
                                "目标显示器已断开或暂不可调节，已切回所有显示器；请再次操作".into(),
                            );
                        }
                        args[0] = format!("--MonitorID={id}");
                    }
                    args
                }
            };
            command(&job.executable, &args, false).map(|_| ())
        })();
        let terminal = if STOP.load(Ordering::Acquire) {
            "cancelled"
        } else if result.is_err() {
            "failed"
        } else if cycle {
            "passed"
        } else {
            "unknown"
        };
        crate::ble::gatt_note(format!("twinkle_brightness operation_id={operation} phase=completed result={terminal} cycle={cycle} brightness_changed=unknown elapsed_ms={}", started.elapsed().as_millis()));
        (job.completion)(result);
    }

    fn list(executable: &PathBuf) -> Result<Vec<Display>, String> {
        let started = Instant::now();
        let text = command(executable, &["--List".into()], true).map_err(|error| {
            crate::ble::gatt_note(
                "twinkle_list phase=completed result=failed reason=cli_capture".into(),
            );
            error
        })?;
        let displays = parse_displays(&text).map_err(|error| {
            crate::ble::gatt_note(
                "twinkle_list phase=completed result=failed reason=invalid_output".into(),
            );
            error
        })?;
        crate::ble::gatt_note(format!("twinkle_list phase=completed result=passed bytes={} controllable_count={} elapsed_ms={}", text.len(), displays.len(), started.elapsed().as_millis()));
        Ok(displays)
    }

    // Drain bounded stdout while waiting, so a full pipe cannot block the CLI.
    // Cancel synchronous reads on timeout/shutdown, including inherited pipe handles.
    fn command(executable: &PathBuf, args: &[String], capture: bool) -> Result<String, String> {
        if STOP.load(Ordering::Acquire) {
            return Err("亮度请求已取消".into());
        }
        let started = Instant::now();
        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::null())
            .stdout(if capture {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "无法向 Twinkle Tray 发送请求".to_owned())?;
        let stop_read = Arc::new(AtomicBool::new(false));
        let reader = if capture {
            let mut stdout = child.stdout.take().unwrap();
            let stop = Arc::clone(&stop_read);
            Some(std::thread::spawn(move || -> Result<Vec<u8>, String> {
                let mut output = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    if stop.load(Ordering::Acquire) {
                        return Err("显示器列表读取已取消".into());
                    }
                    let count = stdout
                        .read(&mut buffer)
                        .map_err(|_| "无法读取显示器列表".to_owned())?;
                    if count == 0 {
                        break;
                    }
                    output.extend_from_slice(&buffer[..count]);
                    if output.len() > MAX_LIST_BYTES {
                        return Err("显示器列表过大，请检查 Twinkle Tray".into());
                    }
                }
                Ok(output)
            }))
        } else {
            None
        };
        let status = loop {
            if STOP.load(Ordering::Acquire) || started.elapsed() >= Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                break Err(if STOP.load(Ordering::Acquire) {
                    "亮度请求已取消"
                } else {
                    "Twinkle Tray 响应超时"
                }
                .to_owned());
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    break if status.success() {
                        Ok(())
                    } else {
                        Err("Twinkle Tray 未接受请求".into())
                    }
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err("无法确认请求的发送状态".into());
                }
            }
        };
        let mut text = String::new();
        if let Some(reader) = reader {
            while status.is_ok()
                && !reader.is_finished()
                && started.elapsed() < Duration::from_secs(5)
                && !STOP.load(Ordering::Acquire)
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            let cancelled = !reader.is_finished();
            if cancelled {
                stop_read.store(true, Ordering::Release);
                // Repeat cancellation to cover a read racing the stop flag.
                while !reader.is_finished() {
                    let _ = unsafe { CancelSynchronousIo(HANDLE(reader.as_raw_handle())) };
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            let output = reader.join().map_err(|_| "显示器列表读取失败".to_owned())?;
            status?;
            if cancelled {
                return Err("显示器列表读取超时或已取消".into());
            }
            text = String::from_utf8(output?).map_err(|_| "显示器列表编码不受支持".to_owned())?;
        } else {
            status?;
        }
        Ok(text)
    }

    pub(super) fn shutdown() {
        STOP.store(true, Ordering::Release);
        let runner = RUNNER
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(runner) = runner {
            let _ = runner.thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::send_input::ScrollDirection;

    #[test]
    fn legacy_all_mapping_follows_the_selected_target_at_execution_time() {
        for direction in [ScrollDirection::Up, ScrollDirection::Down] {
            assert!(matches!(
                brightness_task(direction, 5, None).unwrap(),
                Task::Selected { steps: 5, .. }
            ));
        }
        assert!(
            matches!(brightness_task(ScrollDirection::Up, 5, Some(2)).unwrap(), Task::Arguments(args) if args[0] == "--MonitorNum=2")
        );
        assert!(brightness_task(ScrollDirection::Up, 0, None).is_err());
        assert!(brightness_task(ScrollDirection::Down, 101, None).is_err());
    }

    #[test]
    fn public_cli_arguments_select_all_or_one_monitor_and_signed_offset() {
        assert_eq!(
            arguments(ScrollDirection::Up, 5, None).unwrap(),
            ["--All", "--Offset=5", "--Overlay"]
        );
        assert_eq!(
            arguments(ScrollDirection::Down, 12, Some(2)).unwrap(),
            ["--MonitorNum=2", "--Offset=-12", "--Overlay"]
        );
    }

    #[test]
    fn selected_and_cycle_actions_round_trip_and_never_repeat() {
        use crate::raw_input::RemoteButton;
        use crate::send_input::{ButtonAction, ButtonMappings, ButtonTrigger};
        for text in [
            r#"{"type":"twinkle_cycle_monitor"}"#,
            r#"{"type":"twinkle_brightness_selected","direction":"up","steps":7}"#,
        ] {
            let action: ButtonAction = serde_json::from_str(text).unwrap();
            assert!(!action.allows_repeat());
            let mut mappings = ButtonMappings::default();
            mappings
                .actions
                .entry(RemoteButton::Power)
                .or_default()
                .double = action.clone();
            assert_eq!(
                mappings
                    .normalized()
                    .unwrap()
                    .action_for(RemoteButton::Power, ButtonTrigger::Double),
                action
            );
            assert_eq!(
                serde_json::from_str::<ButtonAction>(&serde_json::to_string(&action).unwrap())
                    .unwrap(),
                action
            );
        }
        for steps in [0, 101] {
            let mut mappings = ButtonMappings::default();
            mappings
                .actions
                .entry(RemoteButton::Power)
                .or_default()
                .single = ButtonAction::TwinkleBrightnessSelected {
                direction: ScrollDirection::Up,
                steps,
            };
            assert!(mappings.normalized().is_err());
        }
    }

    #[test]
    fn invalid_values_never_produce_a_command() {
        for step in [0, 101, u16::MAX] {
            assert!(arguments(ScrollDirection::Up, step, None).is_err());
        }
        assert!(arguments(ScrollDirection::Up, 5, Some(0)).is_err());
    }

    #[test]
    fn mapping_round_trip_and_validation_preserve_brightness_options() {
        use crate::raw_input::RemoteButton;
        use crate::send_input::{ButtonAction, ButtonMappings, ButtonTrigger};
        let action: ButtonAction = serde_json::from_str(
            r#"{"type":"twinkle_brightness","direction":"down","steps":7,"monitor":2}"#,
        )
        .unwrap();
        let mut mappings = ButtonMappings::default();
        mappings.actions.entry(RemoteButton::Up).or_default().long = action.clone();
        let normalized = mappings.normalized().unwrap();
        assert_eq!(
            normalized.action_for(RemoteButton::Up, ButtonTrigger::Long),
            action
        );
        assert!(
            !action.allows_repeat(),
            "CLI calls must not be repeated at keyboard repeat rate"
        );
        let invalid: ButtonAction =
            serde_json::from_str(r#"{"type":"twinkle_brightness","direction":"up","steps":0}"#)
                .unwrap();
        let mut mappings = normalized;
        mappings.actions.get_mut(&RemoteButton::Up).unwrap().long = invalid;
        assert!(mappings.normalized().is_err());
    }
}
