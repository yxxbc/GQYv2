//! 真跑 `gqy setup`（施工 8-11，`docs/blueprint/cli/setup.md`）：帮助页跟着界面语言；不在终端里又没写 `--provider` 的退出码
//! 2，不拉起核心。在终端里走一遍的在 `crates/gqy-cli/tests/setup.rs`（假终端照剧本回）。

mod support;

use std::process::{Command, Output, Stdio};

use gqy_cli::help::{Page, page};
use gqy_cli::language::Language;
use support::{GQY, Home};

/// 在临时的数据根上跑 `gqy setup <args>`，标准输入是空的管道，界面语言是 `lang`。
fn setup(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(GQY)
        .arg("setup")
        .args(args)
        .env("GQY_HOME", home.root.path())
        .envs(support::offline(home.root.path()))
        .env("GQY_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(Stdio::null())
        .output()
        .expect("跑得起来")
}

#[test]
fn the_help_is_the_page_in_the_language() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        let output = setup(&home, lang, &["-h"]);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            page(language, Page::Setup)
        );
    }
}

#[test]
fn outside_a_terminal_it_needs_a_provider() {
    let home = Home::new();
    let output = setup(&home, "zh_CN.UTF-8", &[]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "要在终端里选，或者写 gqy setup --provider <编号>\n"
    );
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
}
