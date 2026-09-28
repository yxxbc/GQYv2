//! 测试计数门禁与基线读写（19 §6.1 第 11 项、§6.2；P00-04）。
//!
//! 规则：`executed = passed + failed`（**不含** ignored）必须 ≥ 平台基线；低于基线报红——
//! “藏测试躲失败”会被拦下（铁律 10）；高于基线是通过，但提示在同一次提交里更新基线（基线只增）。
//! 基线按平台分文件：`xtask/baselines/test-count.<os>.txt`（linux / macos / windows），一行整数。
//! 为什么按平台：`cfg(target_os)` 让三个平台执行的测试集合不同（19 §6.2），Windows 只跑单元测试，
//! 基线自然更低。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:42:25。

use std::path::{Path, PathBuf};

use crate::arch::Violation;

/// 计数门禁的结论（通过时）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    /// 恰好等于基线。
    Equal,
    /// 高于基线：通过，但提示在同一次提交里更新基线。
    Higher {
        /// 实际执行数。
        executed: usize,
        /// 当前基线。
        baseline: usize,
    },
}

/// 基线目录（相对仓库根）。
pub const BASELINE_DIR: &str = "xtask/baselines";

/// 计数门禁：`executed` 低于 `baseline` 返回 `Err`（红）；等于返回 `Ok(Equal)`；
/// 高于返回 `Ok(Higher)`（警告 + 提示更新基线）。
///
/// # Errors
///
/// `executed` 低于 `baseline` 时返回那一条违规（调用方据此报红并退 1）。
pub fn check_count_gate(executed: usize, baseline: usize) -> Result<GateVerdict, Violation> {
    if executed < baseline {
        return Err(Violation::new(
            "test-count",
            format!("平台 {}", std::env::consts::OS),
            format!("执行数（passed + failed）≥ 平台基线 {baseline}（19 §6.1 第 11 项）"),
            format!("实际执行数 {executed}，低于基线 {}", baseline - executed),
        ));
    }
    if executed > baseline {
        return Ok(GateVerdict::Higher { executed, baseline });
    }
    Ok(GateVerdict::Equal)
}

/// 当前平台的基线文件名（`test-count.<os>.txt`，os 取 `linux` / `macos` / `windows`）。
pub fn baseline_file_name() -> String {
    format!("test-count.{}.txt", std::env::consts::OS)
}

/// 当前平台的基线文件路径（`<仓库根>/xtask/baselines/test-count.<os>.txt`）。
pub fn baseline_path(root: &Path) -> PathBuf {
    root.join(BASELINE_DIR).join(baseline_file_name())
}

/// 读基线（一行整数）。
///
/// # Errors
///
/// 基线文件缺失、读失败或不是整数时返回错误（带路径与期望格式）。
pub fn read_baseline(root: &Path) -> anyhow::Result<usize> {
    let path = baseline_path(root);
    let text = std::fs::read_to_string(&path).map_err(|err| {
        anyhow::anyhow!(
            "读基线 {} 失败：{err}（基线应为一行整数，见 19 §6.2）",
            path.display()
        )
    })?;
    let value = text.trim().parse::<usize>().map_err(|err| {
        anyhow::anyhow!(
            "基线 {} 不是一行整数（实际内容：{:?}）：{err}",
            path.display(),
            text.trim()
        )
    })?;
    Ok(value)
}

/// 写基线（一行整数，末尾换行）；作者更新基线时用，`cargo xtask test` 的输出会提示何时需要。
///
/// # Errors
///
/// 建目录或写文件失败时返回错误。
pub fn write_baseline(root: &Path, executed: usize) -> anyhow::Result<PathBuf> {
    let path = baseline_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("建目录 {} 失败：{err}", parent.display()))?;
    }
    std::fs::write(&path, format!("{executed}\n"))
        .map_err(|err| anyhow::anyhow!("写基线 {} 失败：{err}", path.display()))?;
    Ok(path)
}
