//! 从 01 §3 的分层表解析「包 → 层号 → 允许的重依赖」，并核对 `cargo metadata` 的依赖图。
//!
//! 图纸就是真相源：层表不在这里另存副本，`cargo xtask arch` 每次读文档（改图纸即改门禁）；
//! 解析失败报出文档行号，让改图纸的人一眼看出问题（P00-02「风险与回退」）。
//! 规则出处（01 §3）：只允许依赖严格更低层；入口/连接器不得依赖 L1–L4；只有 `gqy-daemon`
//! 可同时依赖 `gqy-engine` 与 `gqy-gateway`；`gqy-connector-sdk` 只依赖 `gqy-protocol`；
//! 重依赖只允许出现在图纸写明允许它的 crate 里。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。

use std::fmt;

use serde_json::Value;

use super::Violation;

/// 层表一行的目标：具体包名，或一类包的前缀（图纸里的「连接器」对应 `gqy-connector-`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerTarget {
    /// 具体包名，例如 `gqy-core`。
    Package(String),
    /// 一类包的前缀，例如 `连接器` → `gqy-connector-`。
    Prefix(String),
}

/// 层表里的一行。
#[derive(Debug, Clone)]
pub struct LayerEntry {
    /// 这一行覆盖的包（或包前缀）。
    pub target: LayerTarget,
    /// 层号：L0 = 0 … L7 = 7。
    pub layer: u8,
    /// 允许出现在该包的「外部重依赖」名单（图纸第三列，括号里的说明已剥掉）。
    pub allowed_heavy_deps: Vec<String>,
    /// 图纸里的行号（1 起），错误定位用。
    pub line: usize,
}

/// 解析好的层表。
#[derive(Debug, Clone, Default)]
pub struct LayerTable {
    entries: Vec<LayerEntry>,
}

impl LayerTable {
    /// 包名 → 层号；`gqy-connector-*` 这类连接器按前缀行归属。
    pub fn layer_of(&self, package: &str) -> Option<u8> {
        self.entry_of(package).map(|entry| entry.layer)
    }

    /// 包名 → 层表条目：先按精确包名匹配，再按前缀匹配（`gqy-connector-sdk` 走精确匹配）。
    pub fn entry_of(&self, package: &str) -> Option<&LayerEntry> {
        self.entries
            .iter()
            .find(|entry| matches!(&entry.target, LayerTarget::Package(name) if name == package))
            .or_else(|| {
                self.entries.iter().find(|entry| {
                    matches!(&entry.target, LayerTarget::Prefix(prefix) if package.starts_with(prefix.as_str()))
                })
            })
    }

    /// 层表条目数（摘要输出用）。
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// 全部条目（测试与排查用）。
    pub fn entries(&self) -> &[LayerEntry] {
        &self.entries
    }
}

/// 图纸解析失败：带文档行号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerParseError {
    line: usize,
    message: String,
}

impl LayerParseError {
    /// 组装错误；`line` 是文档里的行号（1 起）。
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }

    /// 出错的文档行号（1 起）。
    pub fn line(&self) -> usize {
        self.line
    }
}

impl fmt::Display for LayerParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "第 {} 行：{}", self.line, self.message)
    }
}

impl std::error::Error for LayerParseError {}

/// 解析 01 §3 的分层表：定位章节、逐行读表格、抽出包名/层号/允许重依赖。
///
/// 章节标题必须含「分层与依赖方向」；表头必须含「crate」与「允许的外部重依赖」；
/// 数据行必须是 4 列。任何一条不满足都返回带行号的错误——图纸格式是门禁的输入，
/// 改格式就要同步这里（P00-02「风险与回退」）。
///
/// # Errors
///
/// 找不到章节、找不到表头、列数不足、层号不是 `L<数字>`、crate 列没有可识别的包名时返回错误。
pub fn parse_layers(doc: &str) -> Result<LayerTable, LayerParseError> {
    let mut entries = Vec::new();
    let mut in_section = false;
    let mut saw_header = false;

    for (index, raw) in doc.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim_end();

        if line.starts_with("## ") {
            if in_section {
                break; // 章节结束
            }
            if line.contains("分层与依赖方向") {
                in_section = true;
            }
            continue;
        }
        if !in_section || !line.starts_with('|') {
            continue;
        }
        if line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
            continue; // 表头分隔行
        }
        if line.contains("crate") && line.contains("允许的外部重依赖") {
            saw_header = true;
            continue;
        }

        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() < 4 {
            return Err(LayerParseError::new(
                line_no,
                format!(
                    "层表要 4 列（层 / crate / 职责 / 允许的外部重依赖），这里只有 {} 列",
                    cells.len()
                ),
            ));
        }

        let layer = parse_layer_cell(cells[0], line_no)?;
        for target in parse_crate_cell(cells[1], line_no)? {
            entries.push(LayerEntry {
                target,
                layer,
                allowed_heavy_deps: parse_dep_cell(cells[3]),
                line: line_no,
            });
        }
    }

    if !saw_header {
        return Err(LayerParseError::new(
            1,
            "找不到 01 §3 的分层表（章节标题要含「分层与依赖方向」，表头要含「crate」与「允许的外部重依赖」）",
        ));
    }
    Ok(LayerTable { entries })
}

/// 解析层号列：必须是 `L<数字>`（L0…L7）。
fn parse_layer_cell(cell: &str, line: usize) -> Result<u8, LayerParseError> {
    let mut chars = cell.chars();
    match (chars.next(), chars.next()) {
        (Some('L'), Some(digit)) if digit.is_ascii_digit() => Ok(digit as u8 - b'0'),
        _ => Err(LayerParseError::new(
            line,
            format!("层号列应以 L<数字> 开头（L0…L7），实际是「{cell}」"),
        )),
    }
}

/// 解析 crate 列：反引号里的标识符是包名；「连接器」整类归到 `gqy-connector-` 前缀。
fn parse_crate_cell(cell: &str, line: usize) -> Result<Vec<LayerTarget>, LayerParseError> {
    let mut targets = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    for ch in cell.chars() {
        match ch {
            '`' => {
                if in_code && !current.is_empty() {
                    targets.push(LayerTarget::Package(std::mem::take(&mut current)));
                }
                in_code = !in_code;
            }
            _ if in_code => current.push(ch),
            _ => {}
        }
    }
    if cell.replace('`', "").contains("连接器") {
        targets.push(LayerTarget::Prefix("gqy-connector-".to_string()));
    }
    if targets.is_empty() {
        return Err(LayerParseError::new(
            line,
            format!("crate 列既没有反引号包名、也没有「连接器」：{cell}"),
        ));
    }
    Ok(targets)
}

/// 解析「允许的外部重依赖」列：剥掉括号说明，按「、」和逗号切分，取每段开头的 ASCII 包名。
///
/// 这样 `tokio（仅 rt/sync 最小面，供 blocking::run）` → `tokio`、`grep 系 crate` → `grep`；
/// 「—」等占位会得到空列表。
fn parse_dep_cell(cell: &str) -> Vec<String> {
    let mut names = Vec::new();
    for token in strip_parentheticals(cell).split(['、', ',']) {
        let name: String = token
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

/// 去掉中文括号里的说明（含嵌套）。
fn strip_parentheticals(input: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '（' => depth += 1,
            '）' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            _ => out.push(ch),
        }
    }
    out
}

/// 重依赖前缀：命中这些前缀的外部依赖只允许出现在图纸写明允许它的 crate 里（01 §3 末条）。
///
/// 名单与图纸同步维护：新增重依赖时先改 01 §3 的列，再把前缀加到这里（P00-02「风险与回退」）。
const HEAVY_DEP_PREFIXES: &[&str] = &[
    "reqwest",
    "axum",
    "tower",
    "hyper",
    "tokio-rustls",
    "rustls",
    "rcgen",
    "socket2",
    "rust-embed",
    "rusqlite",
    "ratatui",
    "crossterm",
    "tauri",
    "tokio-tungstenite",
    "libc",
    "nix",
    "landlock",
    "globset",
    "grep",
];

/// 开发工具不在层表里：如实豁免（打印在摘要里），不是隐性白名单。
const DEV_TOOL_PACKAGES: &[&str] = &["xtask"];

/// 入口类包（除 `gqy-connector-sdk`，那是库）：不得依赖 L1–L4。
const ENTRY_PACKAGES: &[&str] = &["gqy-tui", "gqy-desktop"];

/// 核对 `cargo metadata --no-deps` 的结果；返回违规列表，空列表 = 通过。
///
/// 检查四条：包都在层表里（`unknown-package`）、依赖只指向更低层（`layer-order`）、
/// 入口类不得依赖 L1–L4（`entry-layer`）、重依赖只在允许它的 crate 里（`heavy-dep`）；
/// 外加两条装配约束（`daemon-assembly`、`connector-sdk-deps`）。
pub fn check_arch(meta: &Value, table: &LayerTable) -> Vec<Violation> {
    let mut violations = Vec::new();
    let Some(packages) = meta["packages"].as_array() else {
        return vec![Violation::new(
            "metadata",
            "cargo metadata",
            "输出里应有 packages 数组",
            "缺失",
        )];
    };

    for package in packages {
        let Some(name) = package["name"].as_str() else {
            continue;
        };
        if DEV_TOOL_PACKAGES.contains(&name) {
            continue;
        }
        let Some(entry) = table.entry_of(name) else {
            violations.push(Violation::new(
                "unknown-package",
                name,
                "包必须出现在 01 §3 层表里（新增 crate 先改图纸）",
                format!("{name} 不在层表内"),
            ));
            continue;
        };

        let deps: Vec<String> = package["dependencies"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|dep| dep["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let is_entry = ENTRY_PACKAGES.contains(&name)
            || (name.starts_with("gqy-connector-") && name != "gqy-connector-sdk");

        for dep in &deps {
            if let Some(dep_layer) = table.layer_of(dep) {
                if dep_layer >= entry.layer {
                    violations.push(Violation::new(
                        "layer-order",
                        format!("{name} -> {dep}"),
                        format!(
                            "只允许依赖严格更低层（本包 L{}，依赖必须低于 L{}）",
                            entry.layer, entry.layer
                        ),
                        format!("{dep} 是 L{dep_layer}"),
                    ));
                }
                if is_entry && (1..=4).contains(&dep_layer) {
                    violations.push(Violation::new(
                        "entry-layer",
                        format!("{name} -> {dep}"),
                        "入口/连接器不得依赖 L1–L4，只能经 gqy-client / gqy-connector-sdk / gqy-protocol 访问运行时",
                        format!("{dep} 是 L{dep_layer}"),
                    ));
                }
            }
            if is_heavy_dep(dep) && !allowed_dep(entry, dep) {
                violations.push(Violation::new(
                    "heavy-dep",
                    format!("{name} -> {dep}"),
                    format!(
                        "重依赖只允许出现在图纸写明允许它的 crate 里（{name} 的允许列：{}）",
                        if entry.allowed_heavy_deps.is_empty() {
                            "—".to_string()
                        } else {
                            entry.allowed_heavy_deps.join("、")
                        }
                    ),
                    format!("{dep} 不在允许列里"),
                ));
            }
        }

        if name != "gqy-daemon"
            && deps.iter().any(|dep| dep == "gqy-engine")
            && deps.iter().any(|dep| dep == "gqy-gateway")
        {
            violations.push(Violation::new(
                "daemon-assembly",
                name,
                "只有 gqy-daemon 可以同时依赖 gqy-engine 与 gqy-gateway（组装只有一份）",
                "同时依赖了 gqy-engine 与 gqy-gateway",
            ));
        }

        if name == "gqy-connector-sdk" {
            for dep in &deps {
                if table.layer_of(dep).is_some() && dep != "gqy-protocol" {
                    violations.push(Violation::new(
                        "connector-sdk-deps",
                        format!("gqy-connector-sdk -> {dep}"),
                        "只允许依赖 gqy-protocol（L0）与 WS 客户端库",
                        format!("依赖了工作区包 {dep}"),
                    ));
                }
            }
        }
    }

    violations
}

/// 是不是重依赖（按前缀判定；`tower` 命中 `tower-http` 这类同族包）。
fn is_heavy_dep(name: &str) -> bool {
    HEAVY_DEP_PREFIXES.iter().any(|prefix| {
        name == *prefix
            || name.starts_with(&format!("{prefix}-"))
            || name.starts_with(&format!("{prefix}_"))
    })
}

/// 该 crate 的允许列里是否覆盖这个依赖（精确名或同族前缀，任一方向）。
fn allowed_dep(entry: &LayerEntry, dep: &str) -> bool {
    entry.allowed_heavy_deps.iter().any(|allowed| {
        dep == allowed
            || dep.starts_with(&format!("{allowed}-"))
            || dep.starts_with(&format!("{allowed}_"))
            || allowed.starts_with(&format!("{dep}-"))
    })
}
