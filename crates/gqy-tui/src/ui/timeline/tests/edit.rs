//! 编辑、写入排成的行（蓝图 `tui.md`「时间线」第 7 条）：一段里只有编辑时收起那一行接上加减的行数；
//! 被拒的算出错（2026-09-30 项目主人：只读时写入被拒，和写成了看起来一模一样）。

use std::time::Instant;

use super::{edit, rows, segment, text, thought};
use crate::core::ToolStatus;
use crate::theme;
use crate::transcript::{StepKind, ToolState};
use crate::ui::test_support::Fixture;

#[test]
fn a_refused_edit_is_an_error_and_adds_no_lines() {
    // 2026-09-30 项目主人：只读时写入被拒，和写成了看起来一模一样。被拒的算出错，不算进编辑、不算加减的行数。
    let f = Fixture::new();
    let t0 = Instant::now();
    let mut refused = edit(t0, 1);
    if let StepKind::Tool { state, .. } = &mut refused.kind {
        *state = ToolState::Done(ToolStatus::Denied);
    }
    assert!(refused.failed(), "被拒的算出错：图标换成叉、整行红");
    let seg = segment(vec![edit(t0, 0), refused.clone()], None);
    let got = rows(0, &seg, &f.ctx());
    assert_eq!(text(&got), ["  Used 1 tool · 1 edit +2 -1 · 1 err · 2s"]);
    let err = got[0]
        .line
        .spans
        .iter()
        .find(|s| s.content == "1 err")
        .unwrap();
    assert_eq!(err.style, theme::error(), "出错的那一格红");
    // 只想了一下、写入被拒：照样写出来，不收成 `Thought for`（真模型实测：原来试过写入这件事都看不见了）。
    let seg = segment(vec![thought(t0, 0), refused], None);
    assert_eq!(
        text(&rows(0, &seg, &f.ctx())),
        ["  Used 1 tool · 1 thought · 1 err · 2s"]
    );
}

#[test]
fn a_folded_edit_carries_its_added_and_removed_lines() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(vec![thought(t0, 0), edit(t0, 1)], None);
    let rows = rows(0, &seg, &f.ctx());
    assert_eq!(text(&rows), ["  Made 1 edit +2 -1 · 1 thought · 2s"]);
    let spans = &rows[0].line.spans;
    assert!(
        spans
            .iter()
            .any(|s| s.content == "+2" && s.style == theme::added())
    );
    assert!(
        spans
            .iter()
            .any(|s| s.content == "-1" && s.style == theme::removed())
    );
}
