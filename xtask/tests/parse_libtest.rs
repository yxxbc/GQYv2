//! 解析器的夹具测试（P00-04「测试与守护」）：内置 libtest 输出样本（真实格式，抄自实测输出）
//! → 断言结构化结果。覆盖 7 类：全通过、失败+`panicked at` 定位、`ignored`、doc 测试、
//! `should_panic`、编译失败（无 `test result` 行）、多二进制拼接。
//! 去掉 `parse_stream` 里对 `FAILED`/`ignored`/`test result` 任一分支的处理，对应断言必然红。
//!
//! 测试放开（P00-03）：集成测试允许 unwrap/expect/panic/索引。
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use xtask::test::parse::{ParseOutcome, Parsed, parse_stream};

/// 把样本切成行序列。
fn to_lines(sample: &str) -> Vec<String> {
    sample.lines().map(str::to_string).collect()
}

/// 解析样本并取出 `Parsed`；解析失败时直接报出实际结果。
fn parse(sample: &str) -> Parsed {
    match parse_stream(&to_lines(sample)) {
        ParseOutcome::Parsed(parsed) => parsed,
        other => panic!("样本应当解析成功，实际：{other:?}"),
    }
}

/// 样本 1：全通过（lib 单元测试）。
const PASSING: &str = r#"     Running unittests src/lib.rs (target/debug/deps/gqy_core-2687966017dc73cd)

running 2 tests
test a ... ok
test b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
"#;

#[test]
fn parses_all_passing() {
    let parsed = parse(PASSING);
    assert_eq!(parsed.suites.len(), 1, "{parsed:?}");
    let suite = &parsed.suites[0];
    assert_eq!(suite.name, "gqy-core (lib)");
    assert_eq!((suite.passed, suite.failed, suite.ignored), (2, 0, 0));
    assert_eq!(suite.elapsed_ms, 10, "finished in 0.01s → 10ms");
    assert!(parsed.failures.is_empty(), "{parsed:?}");
    assert!(parsed.skipped.is_empty(), "{parsed:?}");
}

/// 样本 2：一个失败用例（含 `panicked at` 位置与断言原文）。
const FAILING: &str = r#"     Running unittests src/lib.rs (target/debug/deps/gqy_ledger-0123456789abcdef)

running 2 tests
test plan::tests::tail_order_is_stable ... FAILED
test plan::tests::head_order_is_stable ... ok

failures:

---- plan::tests::tail_order_is_stable stdout ----

thread 'plan::tests::tail_order_is_stable' (12345) panicked at crates/gqy-ledger/src/plan/tests.rs:88:9:
assertion `left == right` failed
  left: [System, History, User, UntrustedTail, TrustedTail, Runtime]
 right: [System, History, UntrustedTail, User, TrustedTail, Runtime]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    plan::tests::tail_order_is_stable

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
"#;

#[test]
fn parses_failure_with_location() {
    let parsed = parse(FAILING);
    assert_eq!(parsed.suites.len(), 1, "{parsed:?}");
    assert_eq!(parsed.suites[0].failed, 1);
    assert_eq!(parsed.failures.len(), 1, "{parsed:?}");

    let failure = &parsed.failures[0];
    assert_eq!(failure.name, "plan::tests::tail_order_is_stable");
    assert_eq!(failure.suite, "gqy-ledger (lib)");
    assert_eq!(failure.crate_name.as_deref(), Some("gqy-ledger"));
    assert_eq!(
        failure.file.as_deref(),
        Some("crates/gqy-ledger/src/plan/tests.rs")
    );
    assert_eq!(failure.line, Some(88));
    assert!(
        failure.message.contains("left: [System, History, User, UntrustedTail"),
        "期望/实际要在原文里：{}",
        failure.message
    );
    assert_eq!(
        failure.rerun(),
        "cargo test -p gqy-ledger plan::tests::tail_order_is_stable -- --exact"
    );
}

/// 样本 3：多二进制拼接（lib + 集成测试）；集成测试沿用最近一个包的包名，`ignored` 进 skipped。
const MULTI_BINARIES: &str = r#"     Running unittests src/lib.rs (target/debug/deps/gqy_provider-bbbbbbbbbbbbbbbb)

running 1 test
test x ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/slow.rs (target/debug/deps/slow-cccccccccccccccc)

running 3 tests
test needs_real_provider ... ignored
test fast_one ... ok
test fast_two ... ok

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out; finished in 0.05s
"#;

#[test]
fn parses_multiple_binaries_and_ignored() {
    let parsed = parse(MULTI_BINARIES);
    assert_eq!(parsed.suites.len(), 2, "{parsed:?}");
    assert_eq!(parsed.suites[0].name, "gqy-provider (lib)");
    assert_eq!(
        parsed.suites[1].name, "gqy-provider (tests/slow.rs)",
        "集成测试的可执行文件名不是包名，应沿用最近一个包名：{parsed:?}"
    );
    assert_eq!(parsed.suites[1].ignored, 1);
    assert_eq!(
        parsed.skipped,
        vec![xtask::test::parse::Skipped {
            name: "needs_real_provider".to_string(),
            reason: None,
        }],
        "{parsed:?}"
    );
}

/// 样本 4：doc 测试（`Doc-tests <包名>` + 带 `(line N)` 的测试名）。
const DOC_TESTS: &str = r#"   Doc-tests gqy_core

running 2 tests
test src/lib.rs - docs (line 3) ... ok
test src/lib.rs - other (line 9) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
"#;

#[test]
fn parses_doc_tests() {
    let parsed = parse(DOC_TESTS);
    assert_eq!(parsed.suites.len(), 1, "{parsed:?}");
    assert_eq!(parsed.suites[0].name, "gqy-core (doc)");
    assert_eq!(parsed.suites[0].passed, 2);
    assert_eq!(parsed.suites[0].elapsed_ms, 100);
}

/// 样本 5：`should_panic` 的用例（libtest 对它与普通通过同形）。
const SHOULD_PANIC: &str = r#"     Running unittests src/lib.rs (target/debug/deps/gqy_tools-dddddddddddddddd)

running 1 test
test panics_as_expected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
"#;

#[test]
fn parses_should_panic_as_passing() {
    let parsed = parse(SHOULD_PANIC);
    assert_eq!(parsed.suites[0].passed, 1, "{parsed:?}");
    assert!(parsed.failures.is_empty(), "{parsed:?}");
}

/// 样本 6：编译失败（整份输出里没有任何 `test result` 行）。
const COMPILE_ERROR: &str = r#"   Compiling gqy-core v0.3.0 (/tmp/repo/crates/gqy-core)
error[E0425]: cannot find value `x` in this scope
 --> crates/gqy-core/src/lib.rs:5:5
  |
5 |     x
  |     ^ not found in this scope

error: could not compile `gqy-core` (lib) due to 1 previous error
"#;

#[test]
fn reports_unrecognized_when_no_test_result() {
    match parse_stream(&to_lines(COMPILE_ERROR)) {
        ParseOutcome::Unrecognized { line, at } => {
            assert!(
                line.contains("could not compile"),
                "要指出最后一条非空行（编译错误）：{line}"
            );
            assert_eq!(at, 8, "行号应为最后一条非空行（1 起）");
        }
        other => panic!("没有 test result 行时必须报无法识别，实际：{other:?}"),
    }
}

/// 样本 7：畸形的 `test result:` 行（缺少 `ignored` 字段）—— 关键行解析不出，不许当通过。
const MALFORMED_RESULT: &str = r#"     Running unittests src/lib.rs (target/debug/deps/gqy_core-eeeeeeeeeeeeeeee)

running 1 test
test a ... ok

test result: ok. 1 passed; 0 failed; finished in 0.00s
"#;

#[test]
fn reports_unrecognized_for_malformed_result_line() {
    match parse_stream(&to_lines(MALFORMED_RESULT)) {
        ParseOutcome::Unrecognized { line, at } => {
            assert!(line.starts_with("test result:"), "{line}");
            assert_eq!(at, 6, "应指向畸形的那一行");
        }
        other => panic!("畸形 result 行必须报无法识别，实际：{other:?}"),
    }
}
