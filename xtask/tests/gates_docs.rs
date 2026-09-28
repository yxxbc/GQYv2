//! 文档门禁的 fixture（P00-03「测试与守护」）：mini crate 里的断链必须让 `cargo doc` 报红；
//! 私有项断链只有在 `--document-private-items` 下才会被检查——这正是 `cargo xtask check --docs`
//! 用的命令行（见 `xtask/src/check.rs` 的 doc 条目）。
//!
//! 测试放开（P00-03）：集成测试允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::Path;
use std::process::Command;

/// 跑与 `cargo xtask check --docs` 相同 RUSTDOCFLAGS 的 `cargo doc`，返回（是否成功, 合并输出）。
fn cargo_doc(dir: &Path, document_private_items: bool) -> (bool, String) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut args = vec!["doc", "--no-deps"];
    if document_private_items {
        args.push("--document-private-items");
    }
    let output = Command::new(cargo)
        .args(&args)
        .env(
            "RUSTDOCFLAGS",
            "-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links",
        )
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTFLAGS")
        .current_dir(dir)
        .output()
        .expect("启动 cargo doc");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

/// 建一个独立 mini crate（自带空 `[workspace]`，不会被外层 workspace 吸进去）。
fn write_fixture(dir: &Path, source: &str) {
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"m\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n",
    )
    .expect("写清单");
    fs::create_dir_all(dir.join("src")).expect("建 src 目录");
    fs::write(dir.join("src/lib.rs"), source).expect("写源码");
}

/// 公开文档里链到不存在的项。
const PUBLIC_BROKEN_SOURCE: &str =
    "//! 见 [不存在的公开项](crate::no_such_item)。\n\npub fn f() {}\n";

/// 私有函数的文档注释里链到不存在的项（只有 `--document-private-items` 才会看它）。
const PRIVATE_BROKEN_SOURCE: &str =
    "//! 占位文档。\n\n/// 见 [不存在的私有项](crate::no_such_private)。\nfn helper() {}\n";

#[test]
fn broken_public_link_fails() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(tmp.path(), PUBLIC_BROKEN_SOURCE);
    let (ok, output) = cargo_doc(tmp.path(), true);
    assert!(!ok, "公开文档断链应当报红：{output}");
    assert!(
        output.contains("unresolved link"),
        "要报 rustdoc 的断链错误（-D broken_intra_doc_links 生效）：{output}"
    );
}

#[test]
fn private_doc_link_only_fails_with_document_private_items() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(tmp.path(), PRIVATE_BROKEN_SOURCE);

    let (with_flag_ok, with_flag_output) = cargo_doc(tmp.path(), true);
    assert!(
        !with_flag_ok,
        "带 --document-private-items 时私有项断链必须报红：{with_flag_output}"
    );
    assert!(
        with_flag_output.contains("unresolved link"),
        "要报 rustdoc 的断链错误（-D broken_intra_doc_links 生效）：{with_flag_output}"
    );

    let (without_flag_ok, without_flag_output) = cargo_doc(tmp.path(), false);
    assert!(
        without_flag_ok,
        "不带 --document-private-items 时不该看到私有项断链（证明该 flag 的必要性）：{without_flag_output}"
    );
}
