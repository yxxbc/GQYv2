//! `log.level` 的每一个选项 `GQY_LOG` 都读得懂：配置里写的和环境变量写的是同一种写法。

use super::*;
use crate::level;

#[test]
fn every_option_is_a_level_gqy_log_understands() {
    let gqy_config::Kind::Option(options) = LogSettings::ITEMS[0].kind else {
        panic!("log.level 是选项");
    };
    for option in options {
        assert_eq!(level(Some(*option)).unknown, None, "{option}");
    }
    let defaults = LogSettings::from(&gqy_config::Values::defaults(LogSettings::ITEMS));
    assert_eq!(
        level(Some(defaults.level.as_str())),
        level(None),
        "默认值和没设 GQY_LOG 一样"
    );
}
