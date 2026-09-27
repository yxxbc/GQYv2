# P00 · 工程地基

> 状态：【规划】。本阶段把仓库从"只有文档"变成"可以照 00-施工总纲 施工"的骨架。各施工单的状态以施工单文件头为唯一真相源；本 README 只写目标、进入/退出条件与清单。
>
> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

## 阶段目标

workspace、lints、CI、架构/体积门禁、测试日志、错误与日志骨架（与施工图 `plan-data` 同文）。做完之后：

- 一个 `cargo build --workspace` 能编译全部 17 个库 crate 空壳；
- 任何人加错依赖方向、写出被禁模式、写出超长文件、漏写文档、`unwrap` 裸奔都会被机器拦住；
- `cargo xtask test` 输出可读摘要与 JSON/Markdown 报告，测试计数不下降；
- 三平台 CI（Linux、macOS、Windows）在 PR 上全绿。

本阶段不产出任何用户可见功能；M1 之前不发布。

## 进入条件

- `docs/designs/00–20` 设计文档完成并已被用户确认；
- 仓库中没有 `Cargo.toml` 与任何运行时代码（现状）；
- 本机有 Rust stable 工具链（≥ 1.85）与网络（首次拉取 crates.io 索引）；
- 三平台 CI 所需的 GitHub 仓库设置（必需检查、分支保护）在 P00-06 完成后由管理员配置。

## 退出条件

- 六张施工单全部“已完成”；
- 仓库根 `Cargo.toml`、`xtask/`、`.github/workflows/ci.yml` 就位，`cargo xtask check` 与 `cargo xtask test` 本地可复现 CI；
- `pr-standards`、`workflow-security` 及 CI 新作业被设为必需检查（管理员操作，P00-06 列出清单）；
- P01 可以直接开始：在 `gqy-core` 里写类型时，fmt / clippy / arch / size / 文档 / 测试计数门禁全部生效。

## 施工单清单与依赖

| 单 | 标题 | 依赖 | 规模 | 状态 |
| --- | --- | --- | --- | --- |
| [P00-01](P00-01-workspace骨架.md) | workspace 骨架 | — | M | 未开始 |
| [P00-02](P00-02-xtask与分层门禁.md) | xtask 与分层门禁 | P00-01 | L | 未开始 |
| [P00-03](P00-03-体积门禁与clippy规则.md) | 体积门禁与 clippy 规则 | P00-01、P00-02 | M | 未开始 |
| [P00-04](P00-04-测试日志与报告.md) | 测试日志与报告 | P00-02 | L | 未开始 |
| [P00-05](P00-05-错误与日志骨架.md) | 错误与日志骨架 | P00-01 | M | 未开始 |
| [P00-06](P00-06-CI与本地检查接入.md) | CI 与本地检查接入 | P00-02、P00-03、P00-04 | M | 未开始 |

依赖图：

```text
P00-01 ─┬─ P00-02 ─┬─ P00-03 ─┐
        │          └─ P00-04 ─┴─ P00-06
        └─ P00-05
```

- P00-03 依赖 P00-02：`size` 与 `check` 子命令挂在 xtask 上（写单时的依赖修正，施工图 `plan-data` 已同步）。
- P00-05 只动 `crates/gqy-core` 与 `crates/gqy-daemon`，与 P00-02/03/04 的文件不重叠，可并行。
- P00-06 是收口单：把 P00-02/03/04 的门禁接进 CI；P00-05 不阻塞它（CI 矩阵的构建会覆盖 gqy-core，但不新增作业）。

## 里程碑

P00 不设里程碑（里程碑见 00-施工总纲 §2：M1 在 P05 末）。

## 遗留与发现

> 施工过程中发现的范围外问题记在这里；每张单写完后如有残留（例如跟随设计的解读、需要用户拍板的点），也记在这里。

写单时发现的拍板点（均已于 2026-09-28 定案，见 `docs/designs/00-设计理念.md` §5）：

1. **`--include-ignored=<tag>` 的实现机制未定**（P00-04）：19 §4.1/§5.1 要求 `cargo xtask test --include-ignored=<tag>` 能按 `#[ignore = "needs: <env>"]` 的标签选择性运行，但 libtest 不支持按 ignore 理由过滤。推荐方案：xtask 先 `--list` 得到用例名，再扫描测试源码里 `#[ignore = "needs:<tag>"]` 与紧随的函数名建立映射（形式固定、可测试）；备选是把 tag 编进测试名。P00-04 先实现不带 tag 的 `--include-ignored`，机制在首个真实 needs 用例出现的 P02-03 落地。**已定（2026-09-28）**：采纳推荐方案（源码扫描映射 + `--skip`），已写进 19 §4.1 与 P00-04。
2. **`gqy-core` 的允许依赖表述**（P00-05）：01 §3 的“允许的外部重依赖”列没有列 `tokio`/`tracing`，而 02 §2 明确 `gqy_core::blocking::run`、19 §3.1 的 span 层级引用它。P00-05 按 02/19 执行（把 tokio/tracing 视为全项目标配而非“重依赖”）。**已定（2026-09-28）**：写进 01 §3 的 L0 行（连同 `jiff`）。
3. **“吞错误”的机器守护**（P00-03，参考 miyu 0-1/0-2）：miyu 把 `let _ =`、`.ok()` 丢弃 `Result` 也纳入 lint；GQYv2 的 19 §6.1 只列了 `unwrap_used` 等。推荐：加 `clippy::let_underscore_must_use`（restriction，deny）；`.ok()` 没有现成 lint，用扫描器规则（与 `task_local!` 同机制）。**已定（2026-09-28）**：`.ok()` 有现成 lint（`clippy::unused_result_ok`，Clippy 1.82+），与 `let_underscore_must_use` 一并进 19 §6.1 与 P00-03。
4. **时间与本地日期换算的实现**（P00-05、P01-01、P11-05）：技术栈未列时间库；10 §4.4 已定“存储一律 Unix 毫秒 UTC”，13 §58 要 RFC 3339 UTC 字符串，备份时间（10 §9.1）与日志文件名要本地时间/日期。推荐：UTC 毫秒为唯一表示；RFC 3339 UTC 自写（约 30 行，参考 miyu 1-1“时间换算自己写”）；本地日期/时区在用到它的单里落地（P11-05 感知）；若时区处理成本过高再引入 `jiff` 并更新 tech-stack。**已定（2026-09-28）**：不自己写——引入 `jiff` 为唯一时间库（Unix 读系统 tzdb、Windows 默认内嵌 `jiff-tzdb`；RFC 3339 / 本地日期 / 时区同源）；已写入 tech-stack 与 01 §3，P00-05/P01-01 按此实施。


