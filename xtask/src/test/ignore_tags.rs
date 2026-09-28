//! `#[ignore = "needs:<tag>"]` 扫描器与 `--include-ignored=<tag>` 的筛选（19 §4.1；P00-04）。
//!
//! 真实环境用例的属性形式固定为 `#[ignore = "needs:<tag>"]`（单标签，tag 用小写字母/数字/连字符）；
//! 运行器用本模块的映射把**其它**标签的 ignored 用例列入 libtest 的 `--skip`，一次 `cargo test`
//! 只跑目标标签与常规用例。`--skip` 是子串匹配：用例名互为子串时会误伤，此时运行器改为逐用例运行
//! 并提示改名（`find_name_conflicts` 给出冲突对）。
//!
//! 扫描器是启发式（逐行、不解析语法）：只认 `#[ignore = "needs:…"]` 后（最多 10 行内、不跨另一个
//! `#[ignore]`）跟函数名的常见写法；形态不合规进 `warnings`，不静默吞掉。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:42:25。

/// 一个带标签的 ignored 用例。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredCase {
    /// 测试函数名（libtest 输出里的名字以它结尾；`--skip` 用它做子串匹配）。
    pub function: String,
    /// 标签（`needs:<tag>` 的 `<tag>`）。
    pub tag: String,
    /// 来源文件（相对仓库根，报告与提示用）。
    pub file: String,
    /// `#[ignore]` 所在行号（1 起）。
    pub line: usize,
}

/// 扫描结果。
#[derive(Debug, Clone, Default)]
pub struct IgnoreTagMap {
    /// 找到的带标签用例（保持扫描顺序）。
    pub cases: Vec<IgnoredCase>,
    /// 形态问题（打印给作者看；不阻塞运行）。
    pub warnings: Vec<String>,
}

/// 扫一批源码（`(相对路径, 内容)`），提取 `#[ignore = "needs:<tag>"]` 与其后的函数名。
pub fn scan_ignore_tags(sources: &[(String, String)]) -> IgnoreTagMap {
    let mut map = IgnoreTagMap::default();
    for (file, source) in sources {
        let lines: Vec<&str> = source.lines().collect();
        for (index, raw) in lines.iter().enumerate() {
            let trimmed = raw.trim();
            if let Some(rest) = trimmed.strip_prefix("#[ignore") {
                let line_no = index + 1;
                match parse_needs_tag(rest) {
                    TagParse::Tag(tag) => match find_function(&lines, index + 1) {
                        Some((_fn_line, function)) => {
                            map.cases.push(IgnoredCase {
                                function,
                                tag,
                                file: file.clone(),
                                line: line_no,
                            });
                        }
                        None => map.warnings.push(format!(
                            "{file}:{line_no}：#{{ignore}} 之后 10 行内找不到函数名，已跳过"
                        )),
                    },
                    TagParse::NotNeeds => {}
                    TagParse::Malformed => map.warnings.push(format!(
                        "{file}:{line_no}：needs:<tag> 形态不合规（tag 要小写字母/数字/连字符），已跳过"
                    )),
                }
            }
        }
    }
    map
}

/// 按标签划分：要跑的（tag 匹配）与要跳的（其它）。
pub fn partition_by_tag(cases: &[IgnoredCase], tag: &str) -> (Vec<IgnoredCase>, Vec<IgnoredCase>) {
    let mut run = Vec::new();
    let mut skip = Vec::new();
    for case in cases {
        if case.tag == tag {
            run.push(case.clone());
        } else {
            skip.push(case.clone());
        }
    }
    (run, skip)
}

/// 名字子串冲突对（要跑的名字, 要跳的名字）：`--skip` 的子串匹配会误伤这些组合。
pub fn find_name_conflicts(run: &[IgnoredCase], skip: &[IgnoredCase]) -> Vec<(String, String)> {
    let mut conflicts = Vec::new();
    for a in run {
        for b in skip {
            if a.function.contains(&b.function) || b.function.contains(&a.function) {
                conflicts.push((a.function.clone(), b.function.clone()));
            }
        }
    }
    conflicts
}

/// `#[ignore` 之后的文本解析结果。
enum TagParse {
    /// 合规的 `= "needs:<tag>"`。
    Tag(String),
    /// 不是 needs 形式（`#[ignore]`、`#[ignore = "别的理由"]`）：按普通 ignore 对待。
    NotNeeds,
    /// 有 needs 前缀但标签形态不合规。
    Malformed,
}

/// 解析 `#[ignore` 之后的文本（如 ` = "needs:net"]`）。
fn parse_needs_tag(rest: &str) -> TagParse {
    let Some(value) = rest.trim_start().strip_prefix('=') else {
        return TagParse::NotNeeds; // 裸 `#[ignore]`
    };
    let value = value.trim();
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|v| v.rsplit_once('"').map(|(inner, _)| inner))
    else {
        return TagParse::Malformed;
    };
    let Some(tag) = inner.strip_prefix("needs:") else {
        return TagParse::NotNeeds;
    };
    if !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        TagParse::Tag(tag.to_string())
    } else {
        TagParse::Malformed
    }
}

/// 从 `start` 行起找函数名（最多 10 行；遇到另一个 `#[ignore]` 就放弃）。
fn find_function(lines: &[&str], start: usize) -> Option<(usize, String)> {
    for (offset, line) in lines.iter().skip(start).take(10).enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[ignore") {
            return None;
        }
        if let Some(name) = extract_fn_name(line) {
            return Some((start + offset, name));
        }
    }
    None
}

/// 从一行里提取函数名（`fn name(`、`pub fn name<T>(`、`async fn name(` 等）。
fn extract_fn_name(line: &str) -> Option<String> {
    let index = line.find("fn ")?;
    let name: String = line[index + 3..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 样本：合规形态（属性与函数之间隔着 `#[test]`）、裸 `#[ignore]` 与别的理由。
    const SAMPLE: &str = r#"
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs:openai"]
    fn real_provider_roundtrip() {
        assert!(true);
    }

    #[test]
    fn normal_case() {}

    #[ignore = "needs:macos"]
    fn macos_only_thing() {}

    #[ignore]
    fn ignored_without_reason() {}

    #[ignore = "flaky: 偶发失败"]
    fn ignored_with_other_reason() {}
}
"#;

    /// 用固定路径扫描样本。
    fn scan(sample: &str) -> IgnoreTagMap {
        scan_ignore_tags(&[(
            "crates/gqy-provider/tests/real.rs".to_string(),
            sample.to_string(),
        )])
    }

    #[test]
    fn scans_tagged_cases() {
        let map = scan(SAMPLE);
        let names: Vec<(&str, &str)> = map
            .cases
            .iter()
            .map(|case| (case.function.as_str(), case.tag.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("real_provider_roundtrip", "openai"),
                ("macos_only_thing", "macos")
            ],
            "{:?}",
            map.cases
        );
        for case in &map.cases {
            let text = SAMPLE.lines().nth(case.line - 1).expect("行号在样本范围内");
            assert!(
                text.trim_start().starts_with("#[ignore"),
                "行号应指向属性行：{text}"
            );
            assert_eq!(case.file, "crates/gqy-provider/tests/real.rs");
        }
        assert!(map.warnings.is_empty(), "{:?}", map.warnings);
    }

    #[test]
    fn ignores_plain_ignore_forms() {
        let map = scan("#[ignore]\nfn a() {}\n\n#[ignore = \"flaky\"]\nfn b() {}\n");
        assert!(map.cases.is_empty(), "{:?}", map.cases);
        assert!(
            map.warnings.is_empty(),
            "普通理由不算形态问题：{:?}",
            map.warnings
        );
    }

    #[test]
    fn warns_on_malformed_tag() {
        let map = scan("#[ignore = \"needs:NET\"]\nfn a() {}\n");
        assert!(map.cases.is_empty(), "{:?}", map.cases);
        assert_eq!(map.warnings.len(), 1, "{:?}", map.warnings);
        assert!(map.warnings[0].contains("形态不合规"), "{:?}", map.warnings);
    }

    #[test]
    fn warns_when_no_function_follows() {
        let map = scan("#[ignore = \"needs:net\"]\n#[ignore = \"needs:other\"]\nfn a() {}\n");
        assert_eq!(map.cases.len(), 1, "{:?}", map.cases);
        assert_eq!(map.cases[0].tag, "other");
        assert_eq!(map.warnings.len(), 1, "{:?}", map.warnings);
    }

    #[test]
    fn partitions_by_tag() {
        let map = scan(SAMPLE);
        let (run, skip) = partition_by_tag(&map.cases, "openai");
        assert_eq!(run.len(), 1, "{run:?}");
        assert_eq!(run[0].function, "real_provider_roundtrip");
        assert_eq!(skip.len(), 1, "{skip:?}");
        assert_eq!(skip[0].function, "macos_only_thing");
    }

    #[test]
    fn detects_substring_conflicts() {
        let run = vec![IgnoredCase {
            function: "net_test".to_string(),
            tag: "a".to_string(),
            file: "f".to_string(),
            line: 1,
        }];
        let skip = vec![IgnoredCase {
            function: "net_test_long".to_string(),
            tag: "b".to_string(),
            file: "f".to_string(),
            line: 2,
        }];
        assert_eq!(
            find_name_conflicts(&run, &skip),
            vec![("net_test".to_string(), "net_test_long".to_string())]
        );

        let skip_ok = vec![IgnoredCase {
            function: "other".to_string(),
            tag: "b".to_string(),
            file: "f".to_string(),
            line: 2,
        }];
        assert!(find_name_conflicts(&run, &skip_ok).is_empty());
    }
}
