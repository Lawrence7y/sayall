//! 诊断日志的追加与轮转：**单个文件不超过 10 MB，超过即生成新文件**（保留最近几份）。
//!
//! 背景（2026-10-08 现场实测）：单机累计的 `sayall-diagnostic.log` 长到 768 MB、
//! 写入速率约 107 MB/天，既打不开也随不了包。要求：任何单个日志文件不得超过
//! 10 MB，超过就换新文件。
//!
//! 设计约束：主程序与提权助手**写同一个文件**（见 LOGGING.md「单一日志文件：
//! 主程序 + 提权助手」），因此这里刻意做到"无长句柄"：
//!
//! - 每次追加都重新 `open(create|append)`。任一进程轮转（重命名）之后，另一方
//!   下一次写入自然落到新文件——不需要跨进程通知，也不会继续写进已被改名的旧文件；
//! - 轮转判定用「当前大小 + 本次长度 > 上限」，因此单个文件不会超过上限一个记录以上；
//! - 轮转文件名带 UTC 时间戳与进程号，两个进程同时轮转也不会互相覆盖。

use std::io::Write as _;
use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::time::{Duration, Instant};

// A bounded queue keeps filesystem stalls out of GATT, hook and mapping threads.
// Paths are captured per record: runtime environment overrides must not stick.
const QUEUE_CAPACITY: usize = 2048;
static WRITER: OnceLock<Option<DiagnosticWriter>> = OnceLock::new();

enum Record {
    Line(PathBuf, String),
    Flush(mpsc::SyncSender<bool>),
}

struct DiagnosticWriter {
    sender: mpsc::SyncSender<Record>,
    dropped: Arc<AtomicU64>,
}

impl DiagnosticWriter {
    fn spawn(
        capacity: usize,
        mut append: impl FnMut(&Path, &str) -> bool + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_dropped = dropped.clone();
        std::thread::Builder::new()
            .name("sayall-diagnostic-writer".into())
            .spawn(move || {
                let mut writes_ok = true;
                while let Ok(record) = receiver.recv() {
                    match record {
                        Record::Line(path, line) => {
                            let lost = worker_dropped.swap(0, Ordering::Relaxed);
                            if lost != 0 {
                                let note = format!("{} pid={} component=diagnostic_log event=queue_overflow dropped={lost}", crate::ble::utc_timestamp(), std::process::id());
                                if !append(&path, &note) {
                                    worker_dropped.fetch_add(lost, Ordering::Relaxed);
                                    writes_ok = false;
                                }
                            }
                            writes_ok &= append(&path, &line);
                        }
                        Record::Flush(ack) => {
                            let _ = ack.send(writes_ok);
                            writes_ok = true;
                        }
                    }
                }
            })?;
        Ok(Self { sender, dropped })
    }

    fn enqueue(&self, path: &Path, line: &str) -> bool {
        match self
            .sender
            .try_send(Record::Line(path.to_path_buf(), line.to_owned()))
        {
            Ok(()) => true,
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    fn flush(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut record = Record::Flush(sender);
        loop {
            match self.sender.try_send(record) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    record = returned;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
            }
        }
        receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(false)
    }
}

pub(crate) fn initialize_writer() -> bool {
    WRITER
        .get_or_init(|| DiagnosticWriter::spawn(QUEUE_CAPACITY, append_line).ok())
        .is_some()
}

/// Accepted is not persisted. Only the writer touches disk; overload is counted.
pub(crate) fn enqueue_line(path: &Path, line: &str) -> bool {
    initialize_writer();
    WRITER
        .get()
        .and_then(Option::as_ref)
        .is_some_and(|writer| writer.enqueue(path, line))
}

/// Drain accepted records before diagnostics are read or before process exit.
/// Never call this on an input/GATT callback thread.
pub fn flush_diagnostic_log(timeout: Duration) -> bool {
    WRITER
        .get()
        .and_then(Option::as_ref)
        .is_none_or(|writer| writer.flush(timeout))
}

/// 单个日志文件的体积上限（10 MB）。
pub const ROTATE_BYTES: u64 = 10 * 1024 * 1024;
/// 轮转文件保留份数（不含当前文件）：超出时删除最旧的。
pub const KEEP_ROTATED: usize = 5;

/// 轮转判定（纯函数，单测覆盖）：当前大小 + 本次长度已经越过上限。
pub fn rotation_due(current_bytes: u64, incoming_bytes: u64, cap: u64) -> bool {
    current_bytes + incoming_bytes > cap
}

/// 追加一行；必要时先轮转。返回是否写入成功。
pub fn append_line(path: &Path, line: &str) -> bool {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let incoming = line.len() as u64 + 1;
    let current = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if rotation_due(current, incoming, ROTATE_BYTES) {
        rotate(path);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return false;
    };
    let mut buf = String::with_capacity(line.len() + 1);
    buf.push_str(line);
    buf.push('\n');
    let ok = file.write_all(buf.as_bytes()).is_ok();
    let _ = file.flush();
    ok
}

/// 探测"这个路径能不能写"（只创建空文件，不写内容）：`initialize_diagnostic_log`
/// 的返回判据用它，避免为了探测往日志里塞一行无意义内容。
pub fn probe(path: &Path) -> bool {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .is_ok()
}

/// 轮转：把当前文件改名成带时间戳与进程号的旧文件，并按份数修剪。
fn rotate(path: &Path) -> Option<PathBuf> {
    let rotated = rotated_path(path, &crate::ble::utc_stamp_compact(), std::process::id());
    if std::fs::rename(path, &rotated).is_err() {
        // 另一进程已经轮转过（或文件根本不存在）：不是错误，继续写新文件即可。
        return None;
    }
    prune(path, KEEP_ROTATED);
    Some(rotated)
}

/// 轮转文件名（纯函数，单测覆盖）：`<主名>-<UTC 时间戳>-p<进程号>.log`。
///
/// 时间戳形如 `20261008T063158984Z`：只含 ASCII 字母与数字，可安全做文件名，
/// 也可以按字典序当时间序排序（修剪时依赖这一点）。
fn rotated_path(path: &Path, stamp: &str, pid: u32) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "sayall-diagnostic".to_string());
    path.with_file_name(format!("{stem}-{stamp}-p{pid}.log"))
}

/// 按份数修剪轮转文件：名字里的 UTC 时间戳字典序 == 时间序，最旧的先删。
fn prune(path: &Path, keep: usize) {
    let Some(dir) = path.parent() else { return };
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let prefix = format!("{stem}-");
    let mut rotated: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate
                .file_name()
                .map(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with(&prefix) && name.ends_with(".log")
                })
                .unwrap_or(false)
        })
        .collect();
    if rotated.len() <= keep {
        return;
    }
    rotated.sort();
    for old in rotated.iter().take(rotated.len() - keep) {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "sayall-diagnostic-log-{tag}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn queued_logging_never_waits_for_disk_and_reports_overflow() {
        use std::sync::{mpsc, Arc, Mutex};
        use std::time::{Duration, Instant};
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let recorded = lines.clone();
        let writer = DiagnosticWriter::spawn(2, move |_, line| {
            if line == "first" {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
            recorded.lock().unwrap().push(line.to_owned());
            true
        })
        .unwrap();
        let path = Path::new("unused.log");
        assert!(writer.enqueue(path, "first"));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        assert!(writer.enqueue(path, "second"));
        assert!(writer.enqueue(path, "third"));
        assert!(!writer.enqueue(path, "overflow"));
        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(!writer.flush(Duration::from_millis(10)));
        release_tx.send(()).unwrap();
        assert!(writer.flush(Duration::from_secs(2)));
        let lines = lines.lock().unwrap();
        let regular: Vec<_> = lines
            .iter()
            .filter(|line| !line.contains("event=queue_overflow"))
            .map(String::as_str)
            .collect();
        assert_eq!(regular, ["first", "second", "third"]);
        assert!(lines
            .iter()
            .any(|line| line.contains("event=queue_overflow dropped=1")));
    }

    #[test]
    fn queued_logging_preserves_destination_and_surfaces_write_failure() {
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let records = Arc::new(Mutex::new(Vec::new()));
        let captured = records.clone();
        let writer = DiagnosticWriter::spawn(8, move |path, line| {
            captured
                .lock()
                .unwrap()
                .push((path.to_path_buf(), line.to_owned()));
            line != "failure"
        })
        .unwrap();
        assert!(writer.enqueue(Path::new("one.log"), "one"));
        assert!(writer.enqueue(Path::new("two.log"), "failure"));
        assert!(!writer.flush(Duration::from_secs(2)));
        assert_eq!(records.lock().unwrap()[1].0, Path::new("two.log"));
        assert!(writer.enqueue(Path::new("two.log"), "recovered"));
        assert!(writer.flush(Duration::from_secs(2)));
    }

    #[test]
    fn real_disk_queue_latency_probe() {
        use std::time::{Duration, Instant};
        let dir = scratch_dir("latency");
        let path = dir.join("sayall-diagnostic.log");
        let writer = DiagnosticWriter::spawn(QUEUE_CAPACITY, append_line).unwrap();
        let line = "component=latency_probe event=sample result=accepted";
        let start = Instant::now();
        for _ in 0..200 {
            assert!(append_line(&path, line));
        }
        let synchronous = start.elapsed();
        let start = Instant::now();
        for _ in 0..200 {
            assert!(writer.enqueue(&path, line));
        }
        let queued = start.elapsed();
        assert!(writer.flush(Duration::from_secs(5)));
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 400);
        println!(
            "log_latency records=200 synchronous_us={} enqueue_us={} persisted=400",
            synchronous.as_micros(),
            queued.as_micros()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rotation_due_only_past_the_cap() {
        assert!(rotation_due(10 * 1024 * 1024, 1, 10 * 1024 * 1024));
        assert!(rotation_due(10 * 1024 * 1024 - 10, 11, 10 * 1024 * 1024));
        assert!(!rotation_due(10 * 1024 * 1024 - 10, 10, 10 * 1024 * 1024));
        assert!(!rotation_due(0, 1, 10 * 1024 * 1024));
    }

    #[test]
    fn rotated_name_carries_timestamp_and_pid() {
        let path = PathBuf::from(r"C:\Logs\sayall-diagnostic.log");
        let rotated = rotated_path(&path, "20261008T063158984Z", 7920);
        assert_eq!(
            rotated.file_name().unwrap().to_string_lossy(),
            "sayall-diagnostic-20261008T063158984Z-p7920.log"
        );
        assert_eq!(rotated.parent().unwrap(), path.parent().unwrap());
    }

    #[test]
    fn append_line_rotates_when_the_file_would_pass_the_cap() {
        let dir = scratch_dir("rotate");
        let path = dir.join("sayall-diagnostic.log");
        // 种一个"差 1 字节到上限"的文件，下一次写入必须触发轮转。
        let seed = vec![b'x'; (ROTATE_BYTES - 1) as usize];
        std::fs::write(&path, &seed).expect("seed log");
        assert!(append_line(&path, "hello"));
        let rotated: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("sayall-diagnostic-"))
            .collect();
        assert_eq!(rotated.len(), 1, "应生成一个新的旧文件：{rotated:?}");
        assert!(std::fs::metadata(&path).unwrap().len() < ROTATE_BYTES);
        let fresh = std::fs::read_to_string(&path).unwrap();
        assert!(fresh.contains("hello"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_only_the_newest_rotated_files() {
        let dir = scratch_dir("prune");
        let path = dir.join("sayall-diagnostic.log");
        std::fs::write(&path, b"current").unwrap();
        for index in 0..8 {
            let name = format!("sayall-diagnostic-2026100{index}T000000000Z-p1.log");
            std::fs::write(dir.join(name), b"old").unwrap();
        }
        prune(&path, 5);
        let mut left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("sayall-diagnostic-"))
            .collect();
        left.sort();
        assert_eq!(left.len(), 5, "只留最近 5 份：{left:?}");
        assert!(left[0].contains("20261003"), "最旧的应被删掉：{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_creates_but_does_not_write() {
        let dir = scratch_dir("probe");
        let path = dir.join("sayall-diagnostic.log");
        assert!(probe(&path));
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
