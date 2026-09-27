# P14-06 · iMessage 连接器

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P14-02、P14-03、P14-04 |
| 设计依据 | designs/16-连接器与外部平台.md §4.2（读 `chat.db`、`attributedBody` 解码、HEIC → JPEG、`osascript` 发送与回查、TCC 与签名（Q-16-3 研究项）、群聊随实测）、§2（v1 Python 桥的教训：绝对路径进启动器、改路径要重授权）、§3.4（附件上限）、§10（失败模式）、§12（`attributedBody` 解码夹具）；designs/00-设计理念.md §5（连接器生命周期：LaunchAgent）；designs/01-总体架构.md §3（L7 连接器）、§7（apps/connectors）；designs/13-网关与API协议.md §8.2（`connector:imessage` 令牌） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `apps/connectors/imessage`（新增二进制 `gqy-connector-imessage`）、`crates/gqy-connector-sdk`（复用）、`tests/`（解码夹具） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`gqy-connector-imessage`（Rust，替换 v1 的 Python 桥）在 macOS 上可用：**只读**打开 `~/Library/Messages/chat.db`（`SQLITE_OPEN_READ_ONLY`，WAL 下需要同目录可读），轮询 `message.ROWID > 水位`（1 秒一次，或监听 `chat.db-wal` 变化后读取），水位持久化在连接器自己的状态文件；跳过自己发的（`is_from_me`）；`attributedBody`（NSKeyedArchiver / typedstream）用最小解码器取正文与引用，以真实样本（脱敏后入库）夹具钉住；图片 HEIC → JPEG 调用系统自带 `sips`，缺失时原样以 `image/heic` 上报（由 daemon 决定是否可用）；发送走 `osascript` 调 Messages 应用（文本与文件），发送后回查 `chat.db` 的 `is_sent` / `error` 决定 `send_result`；「完全磁盘访问」授予连接器二进制本身——TCC 按代码签名身份识别，**ad-hoc 重建每次都要重新授权**（Q-16-3 研究项：发布方式为签名 `.app` + 固定路径 + LaunchAgent，待实测）。

## 范围

- 做：
  - 读路径：水位持久化、`is_from_me` 过滤、行 ID / guid 作为 `event_id`；`attributedBody` 解码器与夹具（空正文 / 纯表情 / 长文本 / 带回复四类）；畸形输入返回错误并计数、不 panic。
  - 附件：`attachment` 表读取（按列名，缺列时报可读错误并降级该事件）；HEIC → JPEG；大小上限照协议（超限丢弃并记日志，事件其余部分照常）。
  - 发送：文本与文件 `osascript`；回查确认；失败与超时映射 `send_result`；不自动重试。
  - 群聊：`chat` 表区分 direct / group，协议层支持 `conversation.kind = group`；实现范围按真机实测决定（v1 未支持），结论写回 16 §4.2。
  - 表情回应（tapback）先忽略并计数（结论写回 16）。
  - 权限与安装：`gqy connector install imessage` 生成 LaunchAgent（固定路径）；FDA 授权步骤与“重建后需重新授权”写进 README 与 doctor；`--check` 报告 `chat.db` 可读性与 `sips` 可用性。
  - 测试：解码夹具与假 `osascript`（测试替身）在 CI 跑；真机步骤标注「需用户环境」。
- 不做：
  - 发布签名 `.app` 方案（Q-16-3 研究项，另单）；
  - 写 `chat.db`（任何代码路径都不允许）；
  - 非 macOS 平台（能力缺失时失败关闭，doctor 说明）；
  - tapback / 撤回的语义映射（先忽略并计数）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `apps/connectors/imessage/src/{main.rs,config.rs}`（新增） | 装配与配置 |
| `apps/connectors/imessage/src/db.rs`（新增） | 只读打开、水位轮询 |
| `apps/connectors/imessage/src/attributed_body.rs`（新增） | typedstream 最小解码器 |
| `apps/connectors/imessage/src/{attachments.rs,send.rs}`（新增） | HEIC 与 osascript |
| `apps/connectors/imessage/tests/fixtures/*`（新增） | 脱敏 `attributedBody` 样本与期望输出 |
| `crates/gqy-connector-sdk/src/quirks.rs`（修改） | iMessage 怪癖声明（待并入 16 后填字段） |

## 接口草案

草案，以实现为准。

```rust
pub struct DecodedBody { pub text: Option<String>, pub reply_guid: Option<String> }

/// NSKeyedArchiver / typedstream 的最小子集；畸形输入返回错误，不 panic。
pub fn decode_attributed_body(blob: &[u8]) -> Result<DecodedBody, DecodeError>;

/// `sips` 缺失时回退：原样上报 `image/heic`，由 daemon 决定是否可用。
pub fn heic_to_jpeg(path: &Path) -> Result<Vec<u8>, AttachmentError>;
```

## 实施步骤

1. 只读打开 `chat.db` 与水位轮询（先写「WAL 库以只读打开」用例）。
2. `attributedBody` 解码器 + 脱敏夹具（四类）。
3. 附件（HEIC、上限、超限丢弃）。
4. 发送与回查；失败 / 超时映射。
5. 群聊实测与结论回填 16 §4.2；`gqy connector install imessage` 接线；`cargo xtask check`；提交：`feat(connector): iMessage 连接器`。

## 测试与守护

- **解码夹具**：四类样本比对（16 §12；去掉解码器，红）；畸形输入不 panic（返回错误并计数）。
- **只读**：断言 `chat.db` 以只读打开（写句柄测试红）；审查确认无任何写库代码路径。
- **发送回查**：假 `osascript` 成功 / 失败 / 超时三路 → `send_result` 映射正确（去掉回查确认，红）。
- **附件**：HEIC 转 JPEG、`sips` 缺失回退 `image/heic`、超限丢弃（三例）。
- **TCC 说明**：README 与 doctor 含“重建后需重新授权”的步骤（文本存在性检查 + 真机步骤）。
- **隔离**：连接器二进制只依赖 SDK / protocol（arch）；daemon 侧 `chat.db` 关键字扫描为 0。
- 先红后绿对照（PR 贴输出）：去掉 `SQLITE_OPEN_READ_ONLY`、去掉回查确认各一次。

## 验收流程

```sh
cargo xtask check
cargo test -p gqy-connector-imessage
# 需用户环境（macOS + 完全磁盘访问）：gqy connector install imessage → 发一条 iMessage 得流式回复；
#   发送失败（退出 Messages 登录）→ send_result{ok:false} 如实回给工具结果
# mock：解码夹具与假 osascript 在 CI 默认跑
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] `chat.db` 只读打开写进测试；无任何写库代码路径
- [ ] 解码夹具进 CI；真机群聊结论写回 16 §4.2
- [ ] 发布签名（Q-16-3）仍为研究项，没有假装已解决；FDA 步骤文档化
- [ ] 失败分类与 16 §10 一致（附件超限不丢其余事件；发送失败不自动重试）

## 风险与回退

- **TCC 授权漂移**：ad-hoc 重建需重新授权；如实提示（doctor + README），失败关闭、不绕。
- **`chat.db` schema 演进**：按列名读取，缺列时报可读错误并降级该事件；夹具随版本更新。
- **回退**：连接器独立进程，停用不影响 daemon 与其他连接器。
