//! `gqy config` 不带子命令时拉起终端界面（施工 8-24，`docs/blueprint/cli/config.md`「怎么走」第 9 条，
//! 2026-10-04 项目主人定）：在终端里时，找主程序旁边的终端界面 `gqy-tui`（Windows 上是 `.exe`），带
//! `--page config` 拉起它，让它在**这个终端里**起来、停在配置页，等到它退出、退出码照它的；不在终端里
//! （被脚本调、接管道）不拉起，照旧印帮助、退出码 2；找不到界面时说怎么装、退出码 1。
//!
//! **不做信号**（2026-10-04 定）：界面没开着时没人收；Windows 上没有这种信号；开着几个推给谁说不清。
//!
//! **找法照 `gqy web`**（`web.rs` 的 `sibling`）：走清单找（`ui.head`、软件包发现）随 M9——那时候
//! `gqy`、`gqy config`、`gqy web` 三个入口一起改成照清单找（施工方案第二节「拆 M9 时另带四样」第 4 条）。

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

use crate::language::Language;
use crate::web::sibling;

/// 不在终端里、或者参数不对：和别处一样是参数不对的退出码。
const MISUSE: u8 = 2;
/// 没装界面、或者拉不起来。
const FAILED: u8 = 1;

/// 在终端里时拉起界面，停在配置页。交回退出码。
///
/// `main` 是主程序自己的位置（照它找旁边的界面），`tty` 是「现在在不在终端里」（测试喂得到）。
/// 不在终端里时：帮助印到 `out`（那是正常的给人看的输出，和 `gqy config -h` 一样），退出码 2。
/// 没装的、拉不起来的：照 `language` 说在 `err` 上，退出码 1。
pub fn head_on(
    tty: bool,
    main: &Path,
    language: Language,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if !tty {
        // 不在终端里（被脚本调、接管道）：不拉起界面，照旧印帮助、退出码 2。
        if out
            .write_all(crate::help::page(language, crate::help::Page::Config).as_bytes())
            .is_err()
        {
            // 标准输出关了：没有别处可说。
        }
        return MISUSE;
    }
    let program = sibling(main, "gqy-tui");
    if !program.is_file() {
        if writeln!(err, "{}", not_installed(language)).is_err() {
            // 标准错误关了：没有别处可说。
        }
        return FAILED;
    }
    // 参数固定：界面一起来就停在配置页（`tui.md`「全屏配置页」第 1 条）。
    match Command::new(&program).args(["--page", "config"]).status() {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            // 被信号杀的（没有 code）算没成。
            .unwrap_or(FAILED),
        Err(error) => {
            if writeln!(err, "gqy config: {}: {error}", program.display()).is_err() {
                // 标准错误关了：没有别处可说。
            }
            FAILED
        }
    }
}

/// 现在在不在终端里：标准输入和标准输出都是终端才算（被接管道、被重定向的不算）。
pub fn in_terminal() -> bool {
    both(io::stdin().is_terminal(), io::stdout().is_terminal())
}

/// 两头**都是**终端才算。抽出来是为了能直接测「一个是、一个不是」那种情形：
/// `in_terminal` 本身拿不到那种输入（测试进程的 tty 是固定的）。
fn both(stdin_tty: bool, stdout_tty: bool) -> bool {
    stdin_tty && stdout_tty
}

/// 没装终端界面：怎么装。
fn not_installed(language: Language) -> &'static str {
    match language {
        Language::Chinese => {
            "没装终端界面。装法：装和 gqy 同一个版本的 gqy-tui 包（Arch：yay -S gqy-tui；Debian、Ubuntu：apt install gqy-tui；Fedora：dnf install gqy-tui；macOS：brew install gqy-tui），它装在 gqy 旁边。"
        }
        Language::English => {
            "The terminal UI is not installed. Install the gqy-tui package of the same version as gqy (Arch: yay -S gqy-tui; Debian, Ubuntu: apt install gqy-tui; Fedora: dnf install gqy-tui; macOS: brew install gqy-tui); it goes next to gqy."
        }
    }
}

#[cfg(test)]
mod tests;
