//! 会起标题的替身（施工 3-8 五补，`probe_title.rs` 用；从 `mod.rs` 分出来，那边放不下了）。

use gqy_kernel::session::{Policy, Titles};
use gqy_kernel::testkit::Stage;

use super::{environment, policy, start};

/// 同 [`super::stage`]，会起标题：试两次、最多 50 个字，照 `gqy-policy` 的出厂数（施工 3-8 五补）。
pub fn titled_stage() -> Stage {
    let titled = || Policy {
        titles: Some(Titles {
            tries: 2,
            chars: 50,
        }),
        ..policy()
    };
    Stage::new(titled, environment(), start())
}
