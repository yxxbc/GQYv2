# P14-05 · QQ（OneBot）连接器

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P14-02、P14-03、P14-04 |
| 设计依据 | designs/16-连接器与外部平台.md §4.3（方案 B：`gqy-connector-onebot` 正向 WS 连 NapCat、断连 1 秒失败、CQ 码解析、`mentions_self` 加法字段）、§3（协议两侧：本单实现平台一侧）、§2（v1 OneBot 断连卡 180 秒的教训）、§5.1（QQ 号是 `sender.id`、昵称不可信）、§6（配额在 daemon 侧）、§10（平台发送失败不自动重试）、§12（OneBot 断连测试与 CQ 夹具）；designs/00-设计理念.md §5（QQ 接入已定：daemon 内没有任何 QQ 代码）；designs/01-总体架构.md §3（`apps/connectors/` 只依赖 SDK / protocol）、§7（L7）；designs/10-存储与数据演进.md §7.6（`connector_grants` 占位——第一版不需要，见 P14 README 复核第 4 条）；designs/13-网关与API协议.md §8.2（`connector:qq` 令牌与 scope） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `apps/connectors/onebot`（新增二进制 `gqy-connector-onebot`）、`crates/gqy-connector-sdk`（复用与 `DriverQuirks` 填空）、`tests/`（假 NapCat） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`gqy-connector-onebot` 独立进程可用：一侧正向 WS 连 NapCat（支持反向 WS 监听作为选项），另一侧用 `gqy-connector-sdk` 连 daemon；OneBot v11 事件翻译为协议字段——`sender.id` 是 QQ 号、`sender.name` 是昵称（不可信）、群角色进 `sender.platform_role`（仅显示）；CQ 码在连接器内解析为平台中立字段（文本、图片、回复、@），@ 机器人本身的片段转 `mentions_self: true`（加法字段，Q-16-5）；出站文本 / 图片 / 文件走 OneBot API，echo 与响应映射为 `send_result`；**断开 OneBot 侧后所有等待中的 API 调用立即失败（1 秒内）**，daemon 侧对应 `send` 得到 `send_result{ok:false, error:"platform_disconnected"}`——v1 卡 180 秒的故障有测试钉住；平台发送失败不自动重试；重连退避与未 ack 重放走 SDK。

## 范围

- 做：
  - 连接器配置（`<data>/connectors/onebot.toml`，0600；不进版本库、不进日志）：NapCat ws URL 与 access token、daemon 地址与 `connector:qq` 令牌、平台 `account`（机器人 QQ 号）；`--check` 自检输出可读结论。
  - 入站：私聊 / 群聊消息事件、群成员角色（owner / admin / member）、`message` 段解析（text / at / image / reply / face 与其他段降级计数）；CQ 码字符串解析（含畸形片段：未闭合、未知标签 → 文本保留 + 日志计数）；`mentions_self` 置位（@ 机器人）；撤回 / 戳一戳等事件忽略或计数（写回 16）。
  - 群历史：NapCat `get_group_msg_history` 或事件流缓存，取最近 N 条给 daemon（上限随 16 补齐，P14 README 复核第 3 条）；只读、不落连接器库。
  - 出站：`send_private_msg` / `send_group_msg`、图片与文件上传；`message_id` 与错误码映射 `send_result`；`req` 去重由 SDK 负责（本单只实现平台调用）。
  - 断连：WS 断开 → 所有等待中的 OneBot API 调用立即失败（1 秒，测试断言）；重连后未 ack 事件重放。
  - 附件：图片下载 → base64 上送（大小与 MIME 照 16 §3.4）；出站图片 / 文件按 daemon 的 `send` 帧。
  - 假 NapCat 夹具 + 真机验收步骤（标注「需用户环境」）：私聊流式回复、群里 @ 触发、非管理员无工具、断连 1 秒失败。
- 不做：
  - daemon 内任何 QQ 代码（红线；arch 与关键字扫描守护）；
  - OneBot 扩展 API（`get_forward_msg`、合并转发等，按需另开单）；
  - `private_initiative`（另单）；QQ 频道 / 其他 NapCat 非 OneBot v11 接口。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `apps/connectors/onebot/src/{main.rs,config.rs}`（新增） | 装配与配置 |
| `apps/connectors/onebot/src/onebot/{events.rs,cq.rs,api.rs}`（新增） | 事件翻译、CQ 解析、API 调用 |
| `apps/connectors/onebot/src/group_history.rs`（新增） | 群历史读取 |
| `apps/connectors/onebot/tests/*`（新增） | 假 NapCat、CQ 夹具、断连 |
| `crates/gqy-connector-sdk/src/quirks.rs`（修改） | OneBot 怪癖声明（待并入 16 后填字段） |

## 接口草案

草案，以实现为准。

```rust
pub enum CqSegment { Text(String), Image { url: String }, At { qq: String }, Reply { id: String }, Other(String) }

/// 纯函数，夹具驱动：未闭合 / 未知标签降级为文本并计数。
pub fn parse_cq(raw: &str) -> Vec<CqSegment>;

/// mentions_self 在此置位；sender.id = QQ 号，sender.name = 昵称（不可信）。
pub fn to_protocol_event(ev: OneBotEvent, self_id: &str) -> EventIn;
```

## 实施步骤

1. 配置与 SDK 装配；`--check`（先写「缺令牌 / 连不上 NapCat」的期望与实际用例）。
2. 事件翻译与 CQ 解析夹具。
3. 出站 API 与 `send_result` 映射。
4. 断连 1 秒失败与重连重放。
5. 假 NapCat 联测与真机步骤执行；`cargo xtask check`；提交：`feat(connector): QQ（OneBot）连接器`。

## 测试与守护

- **断连**：断开 NapCat 侧后 1 秒内所有等待中的 send 失败（16 §12；去掉断连清理，红）。
- **CQ 夹具**：文本 / 图片 / 回复 / @ / 混合 / 畸形片段（未闭合、未知标签）各一例；@ 机器人 → `mentions_self` 置位。
- **身份**：昵称含伪造 XML → 到达 daemon 时信任不变（与 P14-03 联测同夹具）。
- **不重试**：平台返回错误 → `send_result{ok:false}` 且不自动重发（发送计数断言）。
- **重放**：kill daemon → 连接器按退避重连、未 ack 事件重放且经去重后直接 ack（与 P14-01 / P14-04 联测）。
- **隔离**：连接器二进制只依赖 SDK / protocol（arch）；daemon 侧 QQ 关键字扫描为 0。
- 先红后绿对照（PR 贴输出）：去掉断连清理、去掉 CQ 未知标签降级各一次。

## 验收流程

```sh
cargo xtask check
cargo test -p gqy-connector-onebot
# 需用户环境：配好 NapCat 与 connector:qq 令牌 → 私聊机器人得到流式回复；
#   群里 @ 机器人才触发；断开 NapCat 后 1 秒内工具结果报 platform_disconnected
# mock 版本：假 NapCat 走全部用例（CI 默认路径）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] daemon 侧 QQ 关键字扫描为 0（红线测试）
- [ ] CQ 夹具与断连测试进 CI；真机结论（含群历史来源）写回 16 §4.3
- [ ] 令牌与 access token 不进日志与命令行（断言）
- [ ] 群聊只上报字段，触发判定只在 daemon（无第二处规则）

## 风险与回退

- **NapCat 行为漂移**：以实测为准（先读规范再读代码）；差异写进 `DriverQuirks` 而不到处 if。
- **`mentions_self` 未拍板**：按加法字段上报，触发规则按 Q-16-5 推荐；拍板后只改 daemon 侧规则。
- **回退**：连接器独立进程，停用即回到协议层；daemon 无改动。
