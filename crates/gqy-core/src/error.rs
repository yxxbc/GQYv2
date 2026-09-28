//! 跨层的错误分类（00 §3、01 §6；P00-05）。
//!
//! 每个库 crate 定义自己的 `Error`（thiserror）并实现 [`Classified`]，向上层暴露 [`ErrorKind`]；
//! 重试决策、HTTP 状态映射与日志字段都从分类取值，不靠字符串穿透（00 §3）。
//! 分类是稳定接口：新增类别要先改设计文档再扩枚举；枚举标了 `#[non_exhaustive]`，
//! 下游 `match` 必须带兜底分支。
//! 错误文案的约定：写“期望 X，实际 Y”，不写猜测的原因（00 §3「报错说它真正知道的」）。
//! 创建：AI 助手（Cline 会话），2026-09-28 23:03:25。

/// 跨层的错误分类。重试决策、HTTP 状态映射与日志字段都从这里取值，不靠字符串穿透（00 §3）。
///
/// 分类值来自各设计文档引用的失败语义；后续施工单需要新类别时，先改设计文档再扩这个枚举。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// 内部不变量被破坏（不应发生）。报错必须带期望值与实际值。
    Internal,
    /// 输入不合法（参数、配置、协议字段）。
    InvalidInput,
    /// 未找到（会话、文件、记录）。
    NotFound,
    /// 冲突（并发写、状态不允许）。
    Conflict,
    /// 暂时忙碌，可以稍后重试（队列满、锁被占）。
    Busy,
    /// 超时（连接、流空闲、总时长）。
    Timeout,
    /// 被取消。
    Cancelled,
    /// 权限不足（审批拒绝、沙盒拒绝）。
    PermissionDenied,
    /// 能力不可用（平台不支持、沙盒缺失）。
    Unavailable,
    /// 存储错误（SQLite、写队列）。
    Store,
    /// 供应商错误（HTTP 状态、流中断、协议异常），细分见 06。
    Provider,
}

/// 每个 crate 的错误类型实现它，向上层暴露分类。
///
/// 约定：`kind()` 必须是无副作用的纯查询；错误信息写“期望 X，实际 Y”（00 §3）。
pub trait Classified {
    /// 返回这条错误的分类。
    fn kind(&self) -> ErrorKind;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 样例：下游 crate 的错误类型长这样（thiserror + `Classified`）。
    #[derive(Debug, thiserror::Error)]
    enum SampleError {
        #[error("期望配置文件存在，实际 {path} 不存在")]
        MissingConfig { path: String },
        #[error("期望拿到写锁，实际等满 {waited_ms} ms 仍被占用")]
        LockBusy { waited_ms: u64 },
        #[error("供应商返回 503（期望 200），已重试 {retries} 次")]
        ProviderUnavailable { retries: u32 },
    }

    impl Classified for SampleError {
        fn kind(&self) -> ErrorKind {
            match self {
                Self::MissingConfig { .. } => ErrorKind::NotFound,
                Self::LockBusy { .. } => ErrorKind::Busy,
                Self::ProviderUnavailable { .. } => ErrorKind::Provider,
            }
        }
    }

    #[test]
    fn classifies_sample_errors() {
        let cases: Vec<(SampleError, ErrorKind)> = vec![
            (
                SampleError::MissingConfig {
                    path: "~/.gqy2/config.toml".to_string(),
                },
                ErrorKind::NotFound,
            ),
            (SampleError::LockBusy { waited_ms: 200 }, ErrorKind::Busy),
            (
                SampleError::ProviderUnavailable { retries: 3 },
                ErrorKind::Provider,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.kind(), expected, "{error}");
        }
    }

    #[test]
    fn kind_is_copy_and_comparable() {
        // 分类要能进日志字段与 match 分支，接口上必须 Copy + Eq。
        let kind = ErrorKind::Timeout;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn sample_display_states_expected_and_actual() {
        // 00 §3 的写法约定：错误文案写清期望值与实际值。
        let error = SampleError::MissingConfig {
            path: "x.toml".to_string(),
        };
        let text = error.to_string();
        assert!(text.contains("期望"), "{text}");
        assert!(text.contains("实际"), "{text}");
    }
}
