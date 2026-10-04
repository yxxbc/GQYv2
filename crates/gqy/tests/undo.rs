//! 真跑 `gqy undo`（别名 `gqy rewind`）、`gqy restore`（`docs/construction/4-7-gqy undo、gqy redo（下）.md`，改名施工
//! 4-7 补）：说明跟着界面语言；核心在跑的，撤掉上一次 `gqy ask` 的那一轮、再恢复它，几条命令各接对了自己的那一个。`gqy redo` 施工 4-7 再补又有了，是重做（`tests/redo.rs`）。

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gqy_cli::help::{Page, page};
use gqy_cli::language::Language;
use gqy_ipc::connect_or_start;
use support::{GQY, Home, within};

/// 在数据根 `root` 上跑 `gqy <args>`：界面语言是 `lang`。
fn gqy(root: &Path, lang: &str, args: &[&str]) -> Output {
    Command::new(GQY)
        .args(args)
        .env("GQY_HOME", root)
        .envs(support::offline(root))
        .env("GQY_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("跑得起来")
}

/// 在阻塞线程里跑：核心在这个测试的运行时里。
async fn run(root: &Path, args: &'static [&'static str]) -> Output {
    let root: PathBuf = root.to_path_buf();
    tokio::task::spawn_blocking(move || gqy(&root, "zh_CN.UTF-8", args))
        .await
        .expect("没 panic")
}

#[test]
fn the_help_is_the_page_in_the_language() {
    // 帮助页照界面语言，一个字节不差（施工 4-11）。
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for (args, which) in [
            (&["undo", "-h"][..], Page::Undo),
            (&["undo", "--help"], Page::Undo),
            (&["help", "undo"], Page::Undo),
            (&["rewind", "-h"], Page::Undo),
            (&["restore", "-h"], Page::Restore),
            (&["help", "restore"], Page::Restore),
        ] {
            let output = gqy(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, which),
                "{lang} {args:?}"
            );
        }
    }
}

#[tokio::test]
async fn undo_and_restore_the_last_ask() {
    let home = Home::new();
    home.system_config(support::UNUSABLE_MODEL);
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let root = home.root.path().to_path_buf();
    // 配的模型用不了（驱动还没有，施工 8-11 起没配的 `gqy ask` 不造会话）：这一轮说「没有可用的模型」，可也是一轮。
    let asked = run(&root, &["ask", "在吗"]).await;
    assert_eq!(asked.status.code(), Some(5), "{asked:?}");
    let undone = run(&root, &["undo"]).await;
    assert_eq!(undone.status.code(), Some(0), "{undone:?}");
    assert_eq!(
        String::from_utf8_lossy(&undone.stdout),
        "· 撤销「在吗」这一轮\n发下一句之前，可以用 gqy restore 恢复。\n"
    );
    let restored = run(&root, &["restore"]).await;
    assert_eq!(restored.status.code(), Some(0), "{restored:?}");
    assert_eq!(
        String::from_utf8_lossy(&restored.stdout),
        "· 恢复「在吗」这一轮\n"
    );
    // `rewind` 和 `undo` 一样。
    let rewound = run(&root, &["rewind"]).await;
    assert_eq!(rewound.status.code(), Some(0), "{rewound:?}");
    assert!(
        String::from_utf8_lossy(&rewound.stdout).starts_with("· 撤销「在吗」这一轮\n"),
        "{rewound:?}"
    );
    drop(held);
    home.until_stopped().await;
}
