//! 核心那边的消息，交给界面（蓝图 `tui.md`「连核心」）：连上、断开、推送、各种请求的回应。

use super::{
    Card, Choice, Current, EffortList, JobOutput, Limits, Push, Rendered, Report, SessionInfo,
};

/// 核心那边的消息，交给界面。
#[derive(Debug)]
pub enum Update {
    /// 配置页请求的原始回应；拒绝保留核心原话，不另发 Refused。
    SettingsRpc {
        /// 界面的请求编号。
        tag: u64,
        /// 原始 result 或拒绝/失联原因。
        result: Result<serde_json::Value, String>,
    },
    /// 配置流发生变更或重同步，配置页应重读，保留编辑草稿。
    SettingsChanged,
    /// 连上了，会话开好了，带着会话编号。
    Ready(String),
    /// 核心没在跑，也没给 `GQY_CORE_BIN`，拉不起来。
    NoCoreBin,
    /// `GQY_CORE_BIN` 指的程序不存在：路径（蓝图「连核心」第 8 条）。
    Missing(String),
    /// 断开以后又连上了，接着订阅着原来那个会话（第 7 条）。
    Reconnected,
    /// 连不上、握手或开会话被拒：原因。
    Failed(String),
    /// 一条请求被拒绝：原因码（`data.reason`，没有的是 `None`），和核心照握手时的语言说的原话。
    Refused {
        /// 原因码，例如 `nothing_to_unrevert`（`protocol/undo.md`）。
        reason: Option<String>,
        /// 核心的原话。
        message: String,
    },
    /// 核心断开了。
    Disconnected,
    /// 会话的限额（订阅的回应里的 `limits`）：窗口、压缩线，侧边栏和框下面那一行照它写上下文。
    Limits(Limits),
    /// 会话里的事。
    Push(Push),
    /// `/model` 框里的一行行（[`Command::ListChoices`](super::Command::ListChoices) 的回应）；要不到的是空的。
    Choices(Vec<Choice>),
    /// 一个网址的卡片（[`Command::LinkPreview`](super::Command::LinkPreview)）；要不到的是 `None`。
    LinkCard {
        /// 网址。
        url: String,
        /// 卡片。
        card: Option<Card>,
    },
    /// 一个 blob 存成了文件（[`Command::FetchBlob`](super::Command::FetchBlob)）；读不成的是 `None`。
    BlobSaved {
        /// 哪个 blob。
        blob: String,
        /// 存在哪。
        path: Option<std::path::PathBuf>,
    },
    /// 一张 mermaid 图画好了（[`Command::RenderMermaid`](super::Command::RenderMermaid)）；核心拒了、没编进 mermaid 的是 `None`。
    Mermaid {
        /// 源码。
        source: String,
        /// 画好的。
        svg: Option<Rendered>,
    },
    /// `/effort` 框里的几级（[`Command::ListEfforts`](super::Command::ListEfforts) 的回应）；要不到的是空的。
    Efforts(EffortList),
    /// `@` 文件列表的回应（[`Command::Files`](super::Command::Files)）：哪个词问的、回应的 `result`（回了错的是 `None`）。
    Files {
        /// 哪个词问的。
        word: crate::mention::Word,
        /// 回应。
        result: Option<serde_json::Value>,
    },
    /// 换模型成了（[`Command::Configure`](super::Command::Configure) 的回应）：引用。
    Configured(String),
    /// 会话现在用的模型（订阅的回应里的 `model`）。
    CurrentModel(Current),
    /// 冷却着的模型里最早什么时候恢复（[`Command::ListModels`](super::Command::ListModels) 的回应）；没有的是 `None`。
    CoolingUntil(Option<jiff::Timestamp>),
    /// 给人看的字（[`Command::FetchHuman`](super::Command::FetchHuman) 的回应）。
    Human(crate::human::Human),
    /// 配置里界面语言的最终值（`ui.language`：`auto` 或者语言代码）：连上时读一次，别处改了再读（「界面语言」）。
    UiLanguage(String),
    /// 会话列表（[`Command::ListSessions`](super::Command::ListSessions) 的回应）：只有主会话，照核心交回的先后。
    Sessions(Vec<SessionInfo>),
    /// 改名成了（`None` 是去掉了标题）：弹一句提示，标题照推送换（蓝图「改名」第 3 条）。
    Renamed(Option<String>),
    /// 回顾交回的是上一句（`cached`：上次回顾以后没有新内容，核心不推 `session.recapped`），照它画（蓝图「回顾」第 3 条）。
    Recap(String),
    /// 撤销（`restore` 为假）或恢复成了：核心算好的给人看的几样（`protocol/undo.md`）。
    Undone {
        /// 是恢复。
        restore: bool,
        /// 回应里给人看的几样。
        report: Report,
    },
    /// 另外订阅着的会话推来的（推送、限额），带着是哪个会话。
    Elsewhere {
        /// 哪个会话。
        session: String,
        /// 推来的。
        update: Box<Update>,
    },
    /// 说的话、重做没发出去（核心拒了、附件传不上）：和 [`Update::Refused`] 一样的两样；先画进正文的那句要撤掉，
    /// 字放回输入框（「输入框」第 12、13 条）。
    Unsent {
        /// 原因码，例如 `not_redoable`。
        reason: Option<String>,
        /// 核心的原话。
        message: String,
    },
    /// 一条后台命令的输出（[`Command::Output`](super::Command::Output) 的回应）；读不了的（任务没了、是子代理）是 `None`。
    Output {
        /// 哪个会话派的。
        session: String,
        /// 任务编号。
        job: String,
        /// 读到的。
        output: Option<JobOutput>,
    },
}
