//! 日志写侧的集成测试（P00-05「测试与守护」）：用 `logging::build` + thread-local 订阅器，
//! 在临时目录里断言 JSONL 行内容、target 分流（daemon / requests）、级别过滤、队列满丢弃
//! 与按大小轮转。`purge_old_logs` 的边界测试在 `logging/purge.rs` 的单测里。
//!
//! 测试放开（P00-03）：集成测试允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use gqy_daemon::logging::{LoggingGuard, LoggingOptions, build};

/// 建带目录的选项（其余用默认值）。
fn options(dir: &Path) -> LoggingOptions {
    LoggingOptions {
        dir: Some(dir.to_path_buf()),
        ..LoggingOptions::default()
    }
}

/// 在 thread-local 订阅器下写日志；返回的 guard 已完成 shutdown（日志都落盘了）。
fn write_logs(opts: &LoggingOptions, f: impl FnOnce()) -> LoggingGuard {
    let (mut guard, subscriber) = build(opts).expect("build 日志");
    tracing::subscriber::with_default(subscriber, f);
    guard.shutdown();
    guard
}

/// 读目录下所有以 `prefix` 开头的文件的行（按文件名排序，覆盖轮转产生的多个文件）。
fn read_lines(dir: &Path, prefix: &str) -> Vec<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("读目录")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
        .collect();
    paths.sort();
    let mut lines = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path).expect("读日志文件");
        lines.extend(text.lines().map(str::to_string));
    }
    lines
}

/// 把一行 JSON 解析出来（顺便验证它是合法 JSON）。
fn parse(line: &str) -> serde_json::Value {
    serde_json::from_str(line).unwrap_or_else(|err| panic!("不是合法 JSON 行（{err}）：{line}"))
}

#[test]
fn writes_jsonl_lines_to_daemon_file() {
    let tmp = tempfile::tempdir().expect("临时目录");
    let guard = write_logs(&options(tmp.path()), || {
        tracing::info!(target: "gqy_test", answer = 42, "你好，日志");
    });
    assert_eq!(guard.dropped(), 0, "不该丢日志");

    let lines = read_lines(tmp.path(), "daemon.");
    assert_eq!(lines.len(), 1, "{lines:?}");
    let value = parse(&lines[0]);
    assert_eq!(value["level"], "INFO", "{value}");
    assert_eq!(value["target"], "gqy_test", "{value}");
    assert_eq!(value["answer"], 42, "{value}");
    assert!(
        value["message"]
            .as_str()
            .expect("message 是字符串")
            .contains("你好"),
        "{value}"
    );
    assert!(
        value["ts"].as_str().expect("ts 是字符串").contains('T'),
        "ts 应当是 RFC 3339：{value}"
    );
    assert!(
        read_lines(tmp.path(), "requests.").is_empty(),
        "普通 target 不写 requests 文件"
    );
}

#[test]
fn routes_usage_and_context_rewrite_to_requests_file() {
    let tmp = tempfile::tempdir().expect("临时目录");
    let guard = write_logs(&options(tmp.path()), || {
        tracing::info!(target: "gqy::usage", input_total = 100, "用量");
        tracing::info!(target: "gqy::context_rewrite", reason = "compaction", "改写");
        tracing::info!(target: "gqy::other", "普通");
    });
    assert_eq!(guard.dropped(), 0);

    let daemon_lines = read_lines(tmp.path(), "daemon.");
    assert_eq!(daemon_lines.len(), 3, "daemon 文件收全部：{daemon_lines:?}");
    let targets: Vec<String> = read_lines(tmp.path(), "requests.")
        .iter()
        .map(|line| {
            parse(line)["target"]
                .as_str()
                .expect("target 是字符串")
                .to_string()
        })
        .collect();
    assert_eq!(
        targets,
        vec!["gqy::usage", "gqy::context_rewrite"],
        "requests 文件只收这两类 target"
    );
}

#[test]
fn default_filter_drops_debug_events() {
    let tmp = tempfile::tempdir().expect("临时目录");
    let guard = write_logs(&options(tmp.path()), || {
        tracing::info!(target: "gqy_test", "会写");
        tracing::debug!(target: "gqy_test", "不该写");
    });
    assert_eq!(guard.dropped(), 0, "被级别过滤不算丢弃");
    let lines = read_lines(tmp.path(), "daemon.");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("会写"), "{lines:?}");
}

#[test]
fn rotates_when_file_size_exceeds_limit() {
    let tmp = tempfile::tempdir().expect("临时目录");
    let opts = LoggingOptions {
        dir: Some(tmp.path().to_path_buf()),
        max_file_bytes: 200,
        ..LoggingOptions::default()
    };
    let guard = write_logs(&opts, || {
        for index in 0..10 {
            tracing::info!(target: "gqy_test", index, "轮转测试");
        }
    });
    assert_eq!(guard.dropped(), 0, "不该丢日志");

    let names: Vec<String> = std::fs::read_dir(tmp.path())
        .expect("读目录")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();
    assert!(
        names
            .iter()
            .any(|name| name.starts_with("daemon.") && name.ends_with(".1.jsonl")),
        "超限后应当出现 .1.jsonl：{names:?}"
    );
}
