//! 阻塞任务的统一入口（02 §2、19 §3.1；P00-05）。
//!
//! 为什么有它（02 §2）：tokio blocking 池默认上限 64，大文件读写、压缩解压、哈希大 blob、
//! 同步第三方库都要经 [`run`] 包装——统一计时、统一打 `blocking{label}` span，且**不吞错误**：
//! 任务 panic 转成 [`BlockingError::Panicked`]（带 label 与 panic 载荷摘要），runtime 关闭时
//! 转成 [`BlockingError::Cancelled`]，由调用方决定怎么上报。
//! span 是 `debug` 级：默认 `info` 过滤下不出现，排查阻塞任务时把过滤调到 debug 即可（19 §3.2）。
//! `label` 是稳定短标识（如 `store.read`、`blob.hash`），**不得含正文或密钥**（19 §3.1）。
//! 创建：AI 助手（Cline 会话），2026-09-28 23:03:25。

use std::any::Any;
use std::time::Instant;

/// 包装层失败：任务 panic（含 panic 载荷摘要）或 runtime 关闭。
#[derive(Debug, thiserror::Error)]
pub enum BlockingError {
    /// 任务 panic（载荷已取出摘要，避免只剩一个 JoinError 看不出原因）。
    #[error("blocking task `{label}` panicked: {payload}")]
    Panicked {
        /// 调用方给的标签。
        label: &'static str,
        /// panic 载荷摘要（字符串 payload 取原文；其它类型如实标成非字符串）。
        payload: String,
    },
    /// runtime 正在关闭，任务被取消。
    #[error("blocking task `{label}` cancelled: runtime is shutting down")]
    Cancelled {
        /// 调用方给的标签。
        label: &'static str,
    },
}

/// 阻塞任务的统一入口：走 tokio blocking 池（默认上限 64，02 §2），统一计时并打 `blocking{label}` span。
///
/// # Errors
///
/// 任务 panic 时返回 [`BlockingError::Panicked`]；runtime 正在关闭、任务被取消时返回
/// [`BlockingError::Cancelled`]——都不吞。
///
/// # Panics
///
/// 本函数自身不 panic：闭包里的 panic 由 `spawn_blocking` 捕获并转成
/// [`BlockingError::Panicked`]。
pub async fn run<F, R>(label: &'static str, f: F) -> Result<R, BlockingError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let span = tracing::debug_span!("blocking", label = label);
    let started = Instant::now();
    let handle = {
        let span = span.clone();
        tokio::task::spawn_blocking(move || {
            let _entered = span.enter();
            f()
        })
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match handle.await {
        Ok(value) => {
            tracing::debug!(parent: &span, label, elapsed_ms, "阻塞任务完成");
            Ok(value)
        }
        Err(join_error) => {
            if join_error.is_panic() {
                let payload = panic_payload(join_error.into_panic());
                tracing::debug!(parent: &span, label, elapsed_ms, payload = %payload, "阻塞任务 panic");
                Err(BlockingError::Panicked { label, payload })
            } else {
                tracing::debug!(parent: &span, label, elapsed_ms, "阻塞任务被取消");
                Err(BlockingError::Cancelled { label })
            }
        }
    }
}

/// 从 panic 载荷里取可读摘要：字符串原文，其它类型如实标成非字符串。
fn panic_payload(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(text) => *text,
        Err(other) => match other.downcast::<&'static str>() {
            Ok(text) => (*text).to_string(),
            Err(_) => "（非字符串 payload）".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn returns_value() {
        let value = run("test.echo", || 21 * 2).await.expect("应当成功");
        assert_eq!(value, 42);
    }

    #[tokio::test]
    async fn panic_becomes_error_with_label_and_payload() {
        let error = run("test.panic", || panic!("boom：期望 1，实际 2"))
            .await
            .expect_err("panic 必须转成错误");
        match error {
            BlockingError::Panicked { label, payload } => {
                assert_eq!(label, "test.panic");
                assert!(payload.contains("boom"), "{payload}");
            }
            other => panic!("应当是 Panicked，实际：{other:?}"),
        }
    }

    #[tokio::test]
    async fn non_string_panic_payload_is_reported_honestly() {
        let error = run("test.panic-non-string", || std::panic::panic_any(7u8))
            .await
            .expect_err("panic 必须转成错误");
        match error {
            BlockingError::Panicked { payload, .. } => {
                assert_eq!(payload, "（非字符串 payload）");
            }
            other => panic!("应当是 Panicked，实际：{other:?}"),
        }
    }

    /// 收集 `blocking` span 的（名字, `label` 字段）。
    #[derive(Clone, Default)]
    struct SpanCollector {
        spans: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for SpanCollector {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &tracing::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if attrs.metadata().name() != "blocking" {
                return;
            }
            /// 只取 `label` 字段的访问器。
            struct LabelVisitor(String);
            impl tracing::field::Visit for LabelVisitor {
                fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                    if field.name() == "label" {
                        self.0 = value.to_string();
                    }
                }
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    if field.name() == "label" {
                        self.0 = format!("{value:?}");
                    }
                }
            }
            let mut visitor = LabelVisitor(String::new());
            attrs.record(&mut visitor);
            if let Ok(mut guard) = self.spans.lock() {
                guard.push((attrs.metadata().name().to_string(), visitor.0));
            }
        }
    }

    #[tokio::test]
    async fn span_carries_label() {
        use tracing_subscriber::layer::SubscriberExt;

        let collector = SpanCollector::default();
        let spans = collector.spans.clone();
        let subscriber = tracing_subscriber::registry()
            .with(collector)
            .with(tracing_subscriber::filter::LevelFilter::DEBUG);
        let _default = tracing::subscriber::set_default(subscriber);

        let value = run("test.span", || 7).await.expect("应当成功");
        assert_eq!(value, 7);

        let collected = spans.lock().expect("锁可用").clone();
        assert!(
            collected
                .iter()
                .any(|(name, label)| name == "blocking" && label == "test.span"),
            "应当收集到 blocking span：{collected:?}"
        );
    }
}
