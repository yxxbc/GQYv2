//! 日志初始化骨架（19 §3.2；P00-05）。
//!
//! 三个输出（19 §3.2 的表格）：
//! - stderr：人读紧凑格式，级别 ≥ 过滤值（默认 `info`）；
//! - `<dir>/daemon.YYYY-MM-DD.jsonl`：JSON 行，含全部 target；
//! - `<dir>/requests.YYYY-MM-DD.jsonl`：只含 target `gqy::usage` 与 `gqy::context_rewrite`。
//!
//! JSON 行的字段：`ts`（RFC 3339 UTC）、`level`、`target`，加上事件自带的字段（含 `message`）。
//! 这是本模块自定的形状；19 §3.3 的用量行字段是**事件内容**（P02-07 起），不受这里影响。
//!
//! 写侧不阻塞（19 §3.2）：每个文件目标一条有界队列 + 独立写线程；
//! 队列满时 `debug` / `trace` 直接丢弃并计数，`info` 以上等待至多 [`INFO_WAIT`] 后丢弃并计数；
//! 丢弃计数每 [`DROP_REPORT_INTERVAL`] 至少报一行 `warn`（**直接写 stderr**，避免再进日志层造成递归）。
//! 按日轮转：文件名带本地日期；单文件超过 `max_file_bytes`（默认 256 MiB）时当日滚动为
//! `.1.jsonl`、`.2.jsonl`……保留策略见 [`purge_old_logs`]（默认 14 天，daemon 启动时调用，P05-05 接线）。
//!
//! 目录与级别由调用方给出（`GqyPaths` 在 P01-03 才出现）；
//! `GQY2_LOG` 环境变量的接线在 P11-01，本单只接受 [`LoggingOptions::filter`]。
//! 创建：AI 助手（Cline 会话），2026-09-28 23:03:25。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::{Duration, Instant};

use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// `requests.*.jsonl` 只收这两个 target（19 §3.2）。
const REQUESTS_TARGETS: &[&str] = &["gqy::usage", "gqy::context_rewrite"];

/// `info` 以上事件在队列满时等待的上限（19 §3.2）。
const INFO_WAIT: Duration = Duration::from_millis(50);

/// 丢弃计数的上报间隔（19 §3.2 的「每分钟一行 warn」）。
const DROP_REPORT_INTERVAL: Duration = Duration::from_secs(60);

/// 写线程等待新消息的轮询周期（用于响应停机信号）。
const WRITER_POLL: Duration = Duration::from_millis(50);

pub mod purge;

/// 日志初始化参数。缺省值集中在本类型的 [`Default`]（19 §3.2 的 256 MiB 与 8192 只在这里出现一次）。
#[derive(Debug, Clone)]
pub struct LoggingOptions {
    /// 日志目录（通常为 `<数据目录>/logs`）。`None` = 只输出 stderr。
    pub dir: Option<PathBuf>,
    /// 过滤级别，默认 `info`；语法同 `RUST_LOG`（`GQY2_LOG` 接线在 P11-01）。
    pub filter: String,
    /// 单文件上限，默认 256 MiB。
    pub max_file_bytes: u64,
    /// 非阻塞队列容量，默认 8192。
    pub queue_capacity: usize,
}

impl Default for LoggingOptions {
    fn default() -> Self {
        Self {
            dir: None,
            filter: "info".to_string(),
            max_file_bytes: 256 * 1024 * 1024,
            queue_capacity: 8192,
        }
    }
}

/// 日志初始化失败。
#[derive(Debug, thiserror::Error)]
pub enum LoggingInitError {
    /// 建目录或打开文件失败。
    #[error("日志目录不可用（期望可写目录，实际 {path}）：{source}")]
    Io {
        /// 出问题的目录。
        path: PathBuf,
        /// 底层错误。
        source: std::io::Error,
    },
    /// 过滤串非法。
    #[error("日志过滤串非法（期望形如 info,gqy_engine=debug，实际 {filter:?}）：{source}")]
    BadFilter {
        /// 调用方给的过滤串。
        filter: String,
        /// 解析错误。
        source: tracing_subscriber::filter::ParseError,
    },
    /// 进程内已经安装过全局订阅器。
    #[error("全局日志已初始化过（同一进程只能 init 一次）：{source}")]
    AlreadyInitialized {
        /// 底层错误。
        source: tracing_subscriber::util::TryInitError,
    },
}

/// 日志守卫：进程存活期间持有；[`LoggingGuard::shutdown`]（或 Drop）会停掉写线程并冲刷。
#[derive(Debug)]
pub struct LoggingGuard {
    writers: Vec<WriterHandle>,
    dropped: Arc<AtomicU64>,
}

impl LoggingGuard {
    /// 到目前为止被丢弃的日志条数（队列满时的 `debug`/`trace` 与等待超时的 `info` 以上）。
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// 停掉写线程并冲刷：返回后，所有已投递的日志都已落盘。Drop 时自动执行。
    pub fn shutdown(&mut self) {
        for mut handle in self.writers.drain(..) {
            handle.shutdown();
        }
    }
}

impl Drop for LoggingGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 一个日志文件目标的写线程句柄。
#[derive(Debug)]
struct WriterHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl WriterHandle {
    /// 通知写线程停机并等它把队列排空。
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take()
            && join.join().is_err()
        {
            eprintln!("警告：日志写线程异常退出（panic），可能丢日志");
        }
    }
}

/// 组装日志层但不安装为全局：测试用 `tracing::subscriber::set_default` 接它。
///
/// # Errors
///
/// 过滤串非法、日志目录不可创建、文件打不开时返回错误。
pub fn build(
    opts: &LoggingOptions,
) -> Result<(LoggingGuard, Box<dyn tracing::Subscriber + Send + Sync>), LoggingInitError> {
    let filter =
        EnvFilter::try_new(&opts.filter).map_err(|source| LoggingInitError::BadFilter {
            filter: opts.filter.clone(),
            source,
        })?;
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact();

    let dropped = Arc::new(AtomicU64::new(0));
    let mut writers = Vec::new();
    let file_layer = match &opts.dir {
        Some(dir) => {
            std::fs::create_dir_all(dir).map_err(|source| LoggingInitError::Io {
                path: dir.clone(),
                source,
            })?;
            let (daemon, daemon_handle) =
                FileSink::start(dir, "daemon", opts, Arc::clone(&dropped))?;
            let (requests, requests_handle) =
                FileSink::start(dir, "requests", opts, Arc::clone(&dropped))?;
            writers.push(daemon_handle);
            writers.push(requests_handle);
            Some(FileLayer { daemon, requests })
        }
        None => None,
    };

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer);
    Ok((LoggingGuard { writers, dropped }, Box::new(subscriber)))
}

/// 组装并安装为全局订阅器（同一进程只能调用一次）；返回的守卫在退出前持有。
///
/// # Errors
///
/// 同 [`build`]；已经安装过全局订阅器时返回 [`LoggingInitError::AlreadyInitialized`]。
pub fn init(opts: &LoggingOptions) -> Result<LoggingGuard, LoggingInitError> {
    let (guard, subscriber) = build(opts)?;
    subscriber
        .try_init()
        .map_err(|source| LoggingInitError::AlreadyInitialized { source })?;
    Ok(guard)
}

/// `now` 在本地时区下的日期。
pub(crate) fn local_date(now: jiff::Timestamp) -> jiff::civil::Date {
    now.to_zoned(jiff::tz::TimeZone::system()).date()
}

/// 今天的本地日期字符串（`YYYY-MM-DD`，日志文件名用）。
fn today_local() -> String {
    local_date(jiff::Timestamp::now()).to_string()
}

/// 从日志文件名里取日期：`daemon.YYYY-MM-DD.jsonl`、`daemon.YYYY-MM-DD.1.jsonl`、`requests.*`。
pub(crate) fn parse_log_file_date(name: &str) -> Option<jiff::civil::Date> {
    let mut parts = name.split('.');
    let prefix = parts.next()?;
    if prefix != "daemon" && prefix != "requests" {
        return None;
    }
    parts.next()?.parse::<jiff::civil::Date>().ok()
}

/// 文件层：把事件渲染成 JSON 行，投给 `daemon` 与（符合条件的）`requests` 两个目标。
struct FileLayer {
    daemon: FileSink,
    requests: FileSink,
}

impl<S: tracing::Subscriber> Layer<S> for FileLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let target = event.metadata().target();
        let level = *event.metadata().level();
        let line = render_json_line(event);
        // 19 §3.2：requests 文件只收用量与上下文改写两类 target；daemon 文件收全部。
        if REQUESTS_TARGETS.contains(&target) {
            self.requests.send(line.clone(), level);
        }
        self.daemon.send(line, level);
    }
}

/// 渲染一条 JSON 行（末尾带换行）。字段：`ts`（RFC 3339 UTC）、`level`、`target` + 事件字段。
fn render_json_line(event: &tracing::Event<'_>) -> Vec<u8> {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "ts".to_string(),
        serde_json::Value::String(jiff::Timestamp::now().to_string()),
    );
    fields.insert(
        "level".to_string(),
        serde_json::Value::String(event.metadata().level().as_str().to_string()),
    );
    fields.insert(
        "target".to_string(),
        serde_json::Value::String(event.metadata().target().to_string()),
    );
    let mut visitor = JsonVisitor {
        fields: &mut fields,
    };
    event.record(&mut visitor);
    let mut line = serde_json::Value::Object(fields).to_string();
    line.push('\n');
    line.into_bytes()
}

/// 把事件字段写进 JSON 对象的访问器。
struct JsonVisitor<'a> {
    fields: &'a mut serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for JsonVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(field.name().to_string(), value.into());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(field.name().to_string(), value.into());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(field.name().to_string(), value.into());
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.fields.insert(field.name().to_string(), value.into());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }

    /// `message` 等以 `Debug` 记录的值：`fmt::Arguments` 的 `Debug` 就是它的 `Display`，
    /// 所以这里得到的是干净文本（不带引号）。
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(format!("{value:?}")),
        );
    }
}

/// 一个日志文件目标：有界队列 + 独立写线程。
struct FileSink {
    sender: SyncSender<Vec<u8>>,
    dropped: Arc<AtomicU64>,
}

impl FileSink {
    /// 建写线程，返回（sink, 写线程句柄）。
    ///
    /// # Errors
    ///
    /// 起不了写线程时返回错误。
    fn start(
        dir: &Path,
        prefix: &'static str,
        opts: &LoggingOptions,
        dropped: Arc<AtomicU64>,
    ) -> Result<(Self, WriterHandle), LoggingInitError> {
        // 容量 0 的通道是 rendezvous（发送即阻塞），不是要的语义：至少给 1。
        let (sender, receiver) =
            std::sync::mpsc::sync_channel::<Vec<u8>>(opts.queue_capacity.max(1));
        let writer = RotatingWriter::new(dir.to_path_buf(), prefix, opts.max_file_bytes);
        let stop = Arc::new(AtomicBool::new(false));
        let writer_stop = Arc::clone(&stop);
        let writer_dropped = Arc::clone(&dropped);
        let join = std::thread::Builder::new()
            .name(format!("gqy-log-{prefix}"))
            .spawn(move || run_writer(writer, receiver, writer_stop, writer_dropped))
            .map_err(|source| LoggingInitError::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        Ok((
            Self { sender, dropped },
            WriterHandle {
                stop,
                join: Some(join),
            },
        ))
    }

    /// 投递一条日志行；按 19 §3.2 的策略处理队列满：`debug`/`trace` 直接丢，
    /// `info` 以上等待至多 [`INFO_WAIT`] 再丢；两种情况都计入丢弃计数。
    fn send(&self, line: Vec<u8>, level: tracing::Level) {
        let sent = if level <= tracing::Level::DEBUG {
            self.sender.try_send(line).is_ok()
        } else {
            self.send_waiting(line, INFO_WAIT)
        };
        if !sent {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 队列满时轮询等待至多 `wait`：`SyncSender` 的带超时 `send` 在 stable 上不可用
    /// （`send_timeout` 属 unstable 的 `std_internals`），所以用 `try_send` + 1 ms 轮询
    /// 实现同一语义（19 §3.2 的「等待至多 50 ms 后丢弃」）。
    fn send_waiting(&self, mut line: Vec<u8>, wait: Duration) -> bool {
        let deadline = Instant::now() + wait;
        loop {
            match self.sender.try_send(line) {
                Ok(()) => return true,
                Err(TrySendError::Full(returned)) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    line = returned;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(TrySendError::Disconnected(_)) => return false,
            }
        }
    }
}

/// 写线程主体：按日/按大小轮转写文件，收到停机信号且队列排空后退出。
fn run_writer(
    mut writer: RotatingWriter,
    receiver: Receiver<Vec<u8>>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
) {
    let mut last_report = Instant::now();
    let mut last_reported = 0_u64;
    loop {
        match receiver.recv_timeout(WRITER_POLL) {
            Ok(line) => write_line(&mut writer, &line),
            Err(RecvTimeoutError::Timeout) => {
                // 超时说明队列已空；此时收到停机信号就可以干净退出。
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if last_report.elapsed() >= DROP_REPORT_INTERVAL {
            let total = dropped.load(Ordering::Relaxed);
            if total > last_reported {
                // 直接写 stderr：再进日志层会递归。
                eprintln!(
                    "警告：日志丢弃 {} 条（队列满；级别过低或写入太慢）",
                    total - last_reported
                );
                last_reported = total;
            }
            last_report = Instant::now();
        }
    }
    // 停机前排空剩余消息（尽力而为）。
    while let Ok(line) = receiver.try_recv() {
        write_line(&mut writer, &line);
    }
}

/// 写一行；失败时直接写 stderr（写线程不能进日志层，否则递归）。
fn write_line(writer: &mut RotatingWriter, line: &[u8]) {
    if let Err(err) = writer.write_line(line) {
        eprintln!("警告：写日志失败：{err}");
    }
}

/// 按日与按大小轮转的文件写入器。
struct RotatingWriter {
    dir: PathBuf,
    prefix: &'static str,
    max_file_bytes: u64,
    current: Option<CurrentFile>,
}

/// 当前打开的文件。
struct CurrentFile {
    date: String,
    index: u32,
    written: u64,
    file: std::fs::File,
}

impl RotatingWriter {
    /// 建写入器（文件在第一次写入时打开）。
    fn new(dir: PathBuf, prefix: &'static str, max_file_bytes: u64) -> Self {
        Self {
            dir,
            prefix,
            max_file_bytes,
            current: None,
        }
    }

    /// 写一行（含换行符），需要时先轮转。
    fn write_line(&mut self, line: &[u8]) -> std::io::Result<()> {
        let today = today_local();
        let need_rotate = match &self.current {
            None => true,
            Some(current) => current.date != today || current.written >= self.max_file_bytes,
        };
        if need_rotate {
            self.rotate(&today)?;
        }
        if let Some(current) = self.current.as_mut() {
            current.file.write_all(line)?;
            current.written += line.len() as u64;
        }
        Ok(())
    }

    /// 轮转到新文件：同日超限时序号 +1（`.1.jsonl`、`.2.jsonl`…），跨日从 0 开始。
    fn rotate(&mut self, today: &str) -> std::io::Result<()> {
        let index = match &self.current {
            Some(current) if current.date == today => current.index + 1,
            _ => 0,
        };
        let path = if index == 0 {
            self.dir.join(format!("{}.{today}.jsonl", self.prefix))
        } else {
            self.dir
                .join(format!("{}.{today}.{index}.jsonl", self.prefix))
        };
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let written = file.metadata()?.len();
        self.current = Some(CurrentFile {
            date: today.to_string(),
            index,
            written,
            file,
        });
        Ok(())
    }
}
