//! 计数门禁的夹具（P00-04「测试与守护」）：纯函数三档（等于 / 高于 / 低于）与基线读写往返。
//! 把 `check_count_gate` 的比较改成 `>=` 恒真（去掉“低于报红”的分支），三档测试必红。
//!
//! 测试放开（P00-03）：集成测试允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use xtask::test::count_gate::{
    GateVerdict, baseline_file_name, check_count_gate, read_baseline, write_baseline,
};

#[test]
fn equal_to_baseline_passes() {
    assert_eq!(
        check_count_gate(46, 46).expect("等于基线应通过"),
        GateVerdict::Equal
    );
}

#[test]
fn higher_than_baseline_warns_and_passes() {
    let verdict = check_count_gate(50, 46).expect("高于基线应通过");
    assert_eq!(
        verdict,
        GateVerdict::Higher {
            executed: 50,
            baseline: 46
        }
    );
}

#[test]
fn lower_than_baseline_fails_with_expected_and_actual() {
    let violation = check_count_gate(40, 46).expect_err("低于基线必须报红");
    assert_eq!(violation.rule, "test-count");
    assert!(
        violation.expected.contains("46"),
        "期望值要写基线：{violation:?}"
    );
    assert!(
        violation.actual.contains("40"),
        "实际值要写执行数：{violation:?}"
    );
    assert!(
        violation.location.contains(std::env::consts::OS),
        "位置要写平台：{violation:?}"
    );
}

#[test]
fn baseline_round_trips() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    let path = write_baseline(tmp.path(), 46).expect("写基线");
    assert!(path.ends_with(baseline_file_name()), "{path:?}");
    assert_eq!(read_baseline(tmp.path()).expect("读基线"), 46);
}

#[test]
fn missing_baseline_is_a_tool_error() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    let err = read_baseline(tmp.path()).expect_err("缺基线应报错");
    assert!(err.to_string().contains("失败"), "{err}");
}

#[test]
fn malformed_baseline_is_a_tool_error() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_baseline(tmp.path(), 1).expect("写基线");
    std::fs::write(
        xtask::test::count_gate::baseline_path(tmp.path()),
        "不是数字\n",
    )
    .expect("写坏基线");
    let err = read_baseline(tmp.path()).expect_err("坏基线应报错");
    assert!(err.to_string().contains("一行整数"), "{err}");
}
