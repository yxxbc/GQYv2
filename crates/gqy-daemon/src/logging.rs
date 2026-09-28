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

use std::path::PathBuf;

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

/// `now` 在本地时区下的日期。
pub(crate) fn local_date(now: jiff::Timestamp) -> jiff::civil::Date {
    now.to_zoned(jiff::tz::TimeZone::system()).date()
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

pub mod purge;
