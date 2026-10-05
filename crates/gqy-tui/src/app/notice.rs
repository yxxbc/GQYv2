//! 框下面那一行的临时提示（蓝图 `tui.md`「提示」）：一个圆角细边框的小框，一次只有一个，新的顶掉旧的，停一会儿
//! 自己消失。画在 `ui/status.rs`。

use std::time::{Duration, Instant};

use super::App;

/// 一条会自己消失的提示。
#[derive(Debug)]
pub struct Notice {
    /// 显示的字。
    pub text: String,
    /// 什么时候消失。
    pub until: Instant,
    /// 是好消息（复制成了）：绿；别的提示暗。
    pub good: bool,
}

impl App {
    /// 在输入框左上方写一条提示，新的顶掉旧的。
    pub(super) fn hint(&mut self, text: String, good: bool) {
        self.notice = Some(Notice {
            text,
            until: Instant::now() + Duration::from_millis(self.config.layout.notice_ms),
            good,
        });
    }
}
