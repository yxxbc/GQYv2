//! libtest 文本输出的解析器（19 §5.1；P00-04）。
//!
//! 解析对象是 `cargo test` 的稳定文本输出：cargo 的 `Running <描述> (<可执行文件>)` 与
//! `Doc-tests <包名>` 行、libtest 的 `running N tests`、`test <名字> ... ok|FAILED|ignored`、
//! `test result: <状态>. N passed; N failed; N ignored; …; finished in X.XXXs`、
//! 失败块 `---- <名字> stdout ----` 与 `panicked at <文件>:<行>:<列>:`。
//!
//! `parse_stream` 是纯函数（输入行序列、输出结构化结果，不碰 I/O），夹具测试直接喂样本
//! （19 §5.1 要求解析器自带夹具）。判据：
//! - 以 `test result:` 开头却解析不出的行 → `Unrecognized`（指出行号与内容）；
//! - 整份输出里一行 `test result:` 都没有（例如编译失败）→ 同样按 `Unrecognized` 处理并指出
//!   最后一条非空行——不把“解析失败”当“通过”（P00-04「风险与回退」）。
//!
//! 逐用例耗时：stable 的 libtest 不提供（`--report-time` 需要 nightly 与 `-Z unstable-options`），
//! 所以耗时只统计到 suite（测试二进制）粒度，摘要里的“最慢”按 suite 排序（施工单实施记录）。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:42:25。

/// 一个测试二进制（suite）的汇总。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteResult {
    /// 展示名：`<包名> (<描述>)`，如 `gqy-core (lib)`、`xtask (tests/gates_size.rs)`、`gqy-core (doc)`。
    pub name: String,
    /// 通过的用例数。
    pub passed: usize,
    /// 失败的用例数。
    pub failed: usize,
    /// 跳过的用例数（libtest 的 `ignored`）。
    pub ignored: usize,
    /// 该 suite 的用时（毫秒，来自 `finished in X.XXXs`）。
    pub elapsed_ms: u64,
}

/// 一个失败用例的定位信息（19 §5.2：位置、期望/实际、复跑）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// 完整测试名（libtest 输出里的名字）。
    pub name: String,
    /// 所属 suite 的展示名。
    pub suite: String,
    /// 复跑命令用的包名（`cargo test -p <包名>`）；未知时为 `None`。
    pub crate_name: Option<String>,
    /// panic 位置的文件（来自 `panicked at <文件>:<行>:<列>:`）。
    pub file: Option<String>,
    /// panic 位置的行号。
    pub line: Option<u32>,
    /// 失败块原文（期望/实际在原文里；解析器只提取，不猜测，19 §5.2）。
    pub message: String,
}

impl Failure {
    /// 可直接复制的复跑命令（19 §5.2 的“复跑”一行）；包名未知时退回 workspace 级命令。
    pub fn rerun(&self) -> String {
        match &self.crate_name {
            Some(krate) => format!("cargo test -p {krate} {} -- --exact", self.name),
            None => format!("cargo test {} -- --exact", self.name),
        }
    }
}

/// 一个被跳过的用例。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// 完整测试名。
    pub name: String,
    /// 跳过原因（来自源码 `#[ignore = "…"]` 的扫描；解析出来的最初为 `None`）。
    pub reason: Option<String>,
}

/// 解析成功的结果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    /// 每个测试二进制一项（保持输出顺序）。
    pub suites: Vec<SuiteResult>,
    /// 失败用例（保持输出顺序）。
    pub failures: Vec<Failure>,
    /// 被跳过的用例（保持输出顺序）。
    pub skipped: Vec<Skipped>,
}

/// 解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// 解析成功。
    Parsed(Parsed),
    /// 无法判定测试结果：`line` 是出问题的行（或最后一条非空行），`at` 是它的行号（1 起）。
    Unrecognized {
        /// 出问题的行内容。
        line: String,
        /// 行号（1 起）。
        at: usize,
    },
}

/// 解析一份 `cargo test --no-fail-fast` 的输出（stdout 与 stderr 合并后的行序列）。
pub fn parse_stream(lines: &[String]) -> ParseOutcome {
    let mut parsed = Parsed::default();
    let mut current: Option<SuiteBuilder> = None;
    let mut block: Option<BlockBuilder> = None;
    let mut last_crate: Option<String> = None;
    let mut saw_result = false;
    let mut last_non_empty: Option<(usize, String)> = None;

    for (index, raw) in lines.iter().enumerate() {
        let at = index + 1;
        let line = raw.trim_end();
        if !line.trim().is_empty() {
            last_non_empty = Some((at, line.to_string()));
        }

        // 失败块收集：遇到终止行先收尾，再让这一行走正常分支。
        if is_block_terminator(line)
            && let Some(finished) = block.take()
        {
            parsed.failures.push(finished.finish(current.as_ref()));
        }
        if let Some(active) = block.as_mut() {
            active.lines.push(line.to_string());
            continue;
        }

        if let Some((desc, bin)) = parse_running_line(line) {
            if let Some(builder) = current.take() {
                // 没有 test result 行的 suite（异常输出）：按零计数收下，保留名字。
                parsed.suites.push(builder.finish(0, 0, 0, 0));
            }
            let crate_name = if desc.starts_with("unittests ") {
                let name = bin.replace('_', "-");
                last_crate = Some(name.clone());
                Some(name)
            } else {
                // 集成测试的可执行文件名是测试文件名，不是包名：沿用最近一个包的包名。
                last_crate.clone()
            };
            current = Some(SuiteBuilder::new(desc, crate_name));
            continue;
        }

        if let Some(krate) = parse_doc_tests_line(line) {
            if let Some(builder) = current.take() {
                parsed.suites.push(builder.finish(0, 0, 0, 0));
            }
            let name = krate.replace('_', "-");
            last_crate = Some(name.clone());
            current = Some(SuiteBuilder::new("Doc-tests".to_string(), Some(name)));
            continue;
        }

        if let Some((name, status)) = parse_test_progress(line) {
            if status == "ignored" {
                parsed.skipped.push(Skipped {
                    name: name.to_string(),
                    reason: None,
                });
            }
            continue;
        }

        if line.starts_with("test result:") {
            saw_result = true;
            let Some((passed, failed, ignored, elapsed_ms)) = parse_result_line(line) else {
                return ParseOutcome::Unrecognized {
                    line: line.to_string(),
                    at,
                };
            };
            let builder = current.take().unwrap_or_else(SuiteBuilder::unknown);
            parsed
                .suites
                .push(builder.finish(passed, failed, ignored, elapsed_ms));
            continue;
        }

        if let Some(name) = parse_block_header(line) {
            block = Some(BlockBuilder {
                name: name.to_string(),
                lines: Vec::new(),
            });
            continue;
        }
    }

    if let Some(finished) = block.take() {
        parsed.failures.push(finished.finish(current.as_ref()));
    }
    if let Some(builder) = current.take() {
        parsed.suites.push(builder.finish(0, 0, 0, 0));
    }

    if !saw_result {
        let (at, line) = last_non_empty.unwrap_or((0, String::new()));
        return ParseOutcome::Unrecognized { line, at };
    }
    ParseOutcome::Parsed(parsed)
}

/// 正在汇总的一个 suite；`test result:` 行到达时定稿。
struct SuiteBuilder {
    desc: String,
    crate_name: Option<String>,
}

impl SuiteBuilder {
    /// 按 `Running` / `Doc-tests` 行建 builder。
    fn new(desc: String, crate_name: Option<String>) -> Self {
        Self { desc, crate_name }
    }

    /// 没有来源行的裸 `test result:`：如实标成未知来源。
    fn unknown() -> Self {
        Self {
            desc: "(未知来源)".to_string(),
            crate_name: None,
        }
    }

    /// 定稿成 `SuiteResult`。
    fn finish(self, passed: usize, failed: usize, ignored: usize, elapsed_ms: u64) -> SuiteResult {
        SuiteResult {
            name: display_name(&self.desc, self.crate_name.as_deref()),
            passed,
            failed,
            ignored,
            elapsed_ms,
        }
    }
}

/// 正在收集的一个失败块（`---- <名字> stdout ----` 到下一个终止行之间）。
struct BlockBuilder {
    name: String,
    lines: Vec<String>,
}

impl BlockBuilder {
    /// 定稿成 `Failure`：位置从 `panicked at` 行提取，其余原文保留（期望/实际在原文里）。
    fn finish(self, suite: Option<&SuiteBuilder>) -> Failure {
        let message = self.lines.join("\n").trim().to_string();
        let (file, line) = extract_panic_location(&message);
        let (suite_name, crate_name) = match suite {
            Some(builder) => (
                display_name(&builder.desc, builder.crate_name.as_deref()),
                builder.crate_name.clone(),
            ),
            None => (String::new(), None),
        };
        Failure {
            name: self.name,
            suite: suite_name,
            crate_name,
            file,
            line,
            message,
        }
    }
}

/// suite 的展示名：`<包名> (<描述简称>)`，如 `gqy-core (lib)`、`xtask (tests/gates_size.rs)`。
fn display_name(desc: &str, crate_name: Option<&str>) -> String {
    let label = short_label(desc);
    match crate_name {
        Some(krate) => format!("{krate} ({label})"),
        None => label,
    }
}

/// `Running <描述> (<可执行文件>)` → （描述, 可执行文件名去掉目录、扩展名与哈希）。
fn parse_running_line(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix("Running ")?;
    let (desc, tail) = rest.rsplit_once(" (")?;
    let exe = tail.strip_suffix(')')?;
    let exe = exe.strip_suffix(".exe").unwrap_or(exe);
    let file = exe.rsplit(['/', '\\']).next()?;
    Some((desc.trim().to_string(), strip_hash_suffix(file)))
}

/// `Doc-tests <包名>` → 包名。
fn parse_doc_tests_line(line: &str) -> Option<&str> {
    line.trim().strip_prefix("Doc-tests ").map(str::trim)
}

/// `test <名字> ... ok|FAILED|ignored|bench` → （名字, 状态）。
fn parse_test_progress(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("test ")?;
    let (name, status) = rest.rsplit_once(" ... ")?;
    Some((name.trim_end(), status.trim()))
}

/// `test result: <状态>. N passed; N failed; N ignored; …; finished in X.XXXs`
/// → （通过, 失败, 跳过, 毫秒）。
fn parse_result_line(line: &str) -> Option<(usize, usize, usize, u64)> {
    let rest = line.strip_prefix("test result:")?.trim();
    let mut passed = None;
    let mut failed = None;
    let mut ignored = None;
    let mut elapsed_ms = None;
    for part in rest.split(';') {
        let part = part.trim();
        // 第一段形如 `ok. 2 passed` / `FAILED. 1 passed`：先剥状态前缀。
        let part = part
            .strip_prefix("ok. ")
            .or_else(|| part.strip_prefix("FAILED. "))
            .unwrap_or(part);
        if let Some(num) = part.strip_suffix(" passed") {
            passed = num.trim().parse::<usize>().ok();
        } else if let Some(num) = part.strip_suffix(" failed") {
            failed = num.trim().parse::<usize>().ok();
        } else if let Some(num) = part.strip_suffix(" ignored") {
            ignored = num.trim().parse::<usize>().ok();
        } else if let Some(seconds) = part
            .strip_prefix("finished in")
            .and_then(|s| s.trim().strip_suffix('s'))
        {
            elapsed_ms = seconds
                .parse::<f64>()
                .ok()
                .map(|secs| (secs * 1000.0).round() as u64);
        }
    }
    Some((passed?, failed?, ignored?, elapsed_ms.unwrap_or(0)))
}

/// `---- <名字> stdout ----` → 名字。
fn parse_block_header(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("---- ")?;
    let name = rest.strip_suffix(" stdout ----")?;
    Some(name)
}

/// 失败块的终止行：下一个块、失败清单、结果行、cargo 的错误尾、下一个 suite。
fn is_block_terminator(line: &str) -> bool {
    line.starts_with("---- ")
        || line == "failures:"
        || line.starts_with("test result:")
        || line.starts_with("error: test failed")
        || line.starts_with("Running ")
        || line.starts_with("Doc-tests ")
}

/// 从失败块原文里提取 panic 位置（`thread '<名字>' (id) panicked at <文件>:<行>:<列>:`）。
fn extract_panic_location(message: &str) -> (Option<String>, Option<u32>) {
    for line in message.lines() {
        let Some(location) = line.split(" panicked at ").nth(1) else {
            continue;
        };
        let location = location.trim().trim_end_matches(':');
        let mut parts = location.rsplitn(3, ':');
        let _column = parts.next();
        let Some(line_no) = parts.next().and_then(|n| n.trim().parse::<u32>().ok()) else {
            continue;
        };
        return (parts.next().map(str::to_string), Some(line_no));
    }
    (None, None)
}

/// 可执行文件名去掉 `-<哈希>` 后缀（`gqy_core-2687966017dc73cd` → `gqy_core`）。
fn strip_hash_suffix(file: &str) -> String {
    if let Some((stem, suffix)) = file.rsplit_once('-')
        && suffix.len() == 16
        && suffix.chars().all(|c| c.is_ascii_hexdigit())
    {
        return stem.to_string();
    }
    file.to_string()
}

/// 描述行的展示简称：`unittests src/lib.rs` → `lib`、`unittests src/main.rs` → `bin`、
/// `Doc-tests` → `doc`、`unittests src/bin/<名>.rs` → `bin: <名>`，其余原样。
fn short_label(desc: &str) -> String {
    match desc {
        "unittests src/lib.rs" => "lib".to_string(),
        "unittests src/main.rs" => "bin".to_string(),
        "Doc-tests" => "doc".to_string(),
        other if other.starts_with("unittests src/bin/") => {
            format!("bin: {}", &other["unittests src/bin/".len()..])
        }
        other => other.to_string(),
    }
}
