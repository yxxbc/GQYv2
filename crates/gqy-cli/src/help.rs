//! 帮助页（施工 4-11，`docs/blueprint/cli/main.md`「帮助页」）：自己写的，一种语言十三页（施工 3-8 四补加了 `recap`，五补加了 `rename`，施工 8-2 加了 `config`，施工 8-5 加了 `login`、`logout`，施工 8-11 加了 `setup`），编进程序，资源目录找不到
//! 也印得出。主程序把它们交给 clap 的 `override_help`：`-h`、`--help`、`gqy help <子命令>` 印的都是这几页。

use crate::language::Language;

/// 哪一页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// `gqy -h`：一页说全。
    GQY,
    /// `gqy ask -h`。
    Ask,
    /// `gqy undo -h`。
    Undo,
    /// `gqy restore -h`（原来叫 `gqy redo`，施工 4-7 补改名）。
    Restore,
    /// `gqy redo -h`（施工 4-7 再补：重做最后一轮，名字收回来了）。
    Redo,
    /// `gqy compact -h`（施工 6-8）。
    Compact,
    /// `gqy recap -h`（施工 3-8 四补）。
    Recap,
    /// `gqy rename -h`（施工 3-8 五补）。
    Rename,
    /// `gqy config -h`，`gqy config get -h` 这几个子命令也印它（施工 8-2）。
    Config,
    /// `gqy login -h`（施工 8-5）。
    Login,
    /// `gqy logout -h`（施工 8-5）。
    Logout,
    /// `gqy sandbox -h`，`gqy sandbox setup -h`、`gqy sandbox remove -h` 也印它（施工 5-8）。
    Sandbox,
    /// `gqy setup -h`（施工 8-11）。
    Setup,
    /// `gqy web -h`（施工 W-9）。
    Web,
}

/// 这种语言的这一页，以一个换行结尾。
pub fn page(language: Language, page: Page) -> &'static str {
    match (language, page) {
        (Language::Chinese, Page::GQY) => include_str!("help/zh/gqy.txt"),
        (Language::Chinese, Page::Ask) => include_str!("help/zh/ask.txt"),
        (Language::Chinese, Page::Undo) => include_str!("help/zh/undo.txt"),
        (Language::Chinese, Page::Restore) => include_str!("help/zh/restore.txt"),
        (Language::Chinese, Page::Redo) => include_str!("help/zh/redo.txt"),
        (Language::Chinese, Page::Compact) => include_str!("help/zh/compact.txt"),
        (Language::Chinese, Page::Recap) => include_str!("help/zh/recap.txt"),
        (Language::Chinese, Page::Rename) => include_str!("help/zh/rename.txt"),
        (Language::English, Page::GQY) => include_str!("help/en/gqy.txt"),
        (Language::English, Page::Ask) => include_str!("help/en/ask.txt"),
        (Language::English, Page::Undo) => include_str!("help/en/undo.txt"),
        (Language::English, Page::Restore) => include_str!("help/en/restore.txt"),
        (Language::English, Page::Redo) => include_str!("help/en/redo.txt"),
        (Language::English, Page::Compact) => include_str!("help/en/compact.txt"),
        (Language::English, Page::Recap) => include_str!("help/en/recap.txt"),
        (Language::English, Page::Rename) => include_str!("help/en/rename.txt"),
        (Language::Chinese, Page::Config) => include_str!("help/zh/config.txt"),
        (Language::English, Page::Config) => include_str!("help/en/config.txt"),
        (Language::Chinese, Page::Login) => include_str!("help/zh/login.txt"),
        (Language::English, Page::Login) => include_str!("help/en/login.txt"),
        (Language::Chinese, Page::Logout) => include_str!("help/zh/logout.txt"),
        (Language::English, Page::Logout) => include_str!("help/en/logout.txt"),
        (Language::Chinese, Page::Sandbox) => include_str!("help/zh/sandbox.txt"),
        (Language::English, Page::Sandbox) => include_str!("help/en/sandbox.txt"),
        (Language::Chinese, Page::Setup) => include_str!("help/zh/setup.txt"),
        (Language::English, Page::Setup) => include_str!("help/en/setup.txt"),
        (Language::Chinese, Page::Web) => include_str!("help/zh/web.txt"),
        (Language::English, Page::Web) => include_str!("help/en/web.txt"),
    }
}

#[cfg(test)]
mod tests;
