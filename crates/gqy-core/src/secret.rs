//! 不会泄漏到日志与调试输出里的密钥容器（19 §3.5；P00-05）。
//!
//! 规则（19 §3.5）：
//! - `Debug` / `Display` 一律输出 `***`；
//! - 不实现 `Serialize`（写配置文件走专门路径）；
//! - 取值必须显式调用 [`Secret::expose`]，便于全局搜索使用点。
//!
//! 守护：canary 单测断言格式化输出不含明文——把 `Debug` 换成 derive 该测试立即红（19 §3.5）。
//! 创建：AI 助手（Cline 会话），2026-09-28 23:03:25。

/// 不会泄漏到日志/调试输出里的密钥容器。
///
/// 用法：配置加载时立刻包进来，其余代码只传 `Secret<T>`；确实要用明文时显式 [`Secret::expose`]。
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// 包进容器（明文只在构造与 [`Secret::expose`] 处出现）。
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// 显式取出明文引用。调用点越少越好（评审时按这个函数名搜索全部使用点）。
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> std::fmt::Debug for Secret<T> {
    /// 任何内容都只输出 `***`（19 §3.5）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

impl<T> std::fmt::Display for Secret<T> {
    /// 任何内容都只输出 `***`（19 §3.5）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 金丝雀明文：出现在任何输出里都算泄漏（19 §3.5）。
    const CANARY: &str = "sk-test-CANARY-123";

    #[test]
    fn debug_and_display_never_leak_plaintext() {
        let secret = Secret::new(CANARY.to_string());
        for rendered in [format!("{secret:?}"), format!("{secret}")] {
            assert_eq!(rendered, "***", "渲染结果：{rendered}");
            assert!(!rendered.contains(CANARY), "泄漏了明文：{rendered}");
        }
        // 嵌套进别的结构体的 Debug 也不泄漏。
        let nested = format!("{:?}", Some(&secret));
        assert!(!nested.contains(CANARY), "嵌套 Debug 泄漏了明文：{nested}");
    }

    #[test]
    fn expose_returns_plaintext() {
        let secret = Secret::new(CANARY.to_string());
        assert_eq!(secret.expose(), CANARY);
    }
}
