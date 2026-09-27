# P05-11 · gqy ask 与 JSONL 契约

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P05-09 |
| 设计依据 | designs/13-网关与API协议.md §9.1（三种输出格式与 `AskLine`）、§9.2（退出码表）、§9.3（非交互场景的问题）、§8.3（`--approve=prompt` 需要 TTY）；designs/08-权限审批与沙盒.md §5.5（非交互场所：Ask 自动 Dismiss、Approval 自动 Deny）；designs/14-TUI.md 的 CLI 约定（命令名与 `-c`；细节以 13 §9 为准） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `apps/gqy`（`src/cmd/ask.rs`）、`crates/gqy-client`、`crates/gqy-protocol`（`AskLine` 复用） |

## 目标

`gqy ask` 成为对外**稳定契约**：`--output-format text|json|stream-json`、`--timeout`、`--session`/`-c`、`--interactive`/`--approve`；退出码 0/1/2/3/124/130；每行 JSON 固定 `"v": 1`、字段只增、最后一行为 `done` 或 `error`；非交互场所的问题按 08 §5.5 自动处理并在 `notice` 里如实说明。

## 范围

- 做：
  - 参数（clap）：问题文本（argv 或 `--stdin`）、`--session <id>` / `-c`（续接终端集成会话）、`--output-format`、`--timeout <dur>`、`--model <name>`（仅本回合）、`--interactive` / `--approve=prompt`（需要 TTY，08 §5.3）。
  - 流程：`gqy-client` 连 daemon → 无会话则建会话（场所 `Cli`）→ 提交回合 → 订阅事件 → 渲染；`text` 走终端渲染、`json` 只打一行终态、`stream-json` 逐事件一行。
  - `AskLine` 由 `gqy_protocol::ask_line_from_event` 派生（唯一函数，P05-01）；`done` / `error` 必在最后一行。
  - 退出码：0 成功；1 回合失败或断连；2 用法错误或协议大版本不匹配；3 会话不存在；124 `--timeout` 到点（先发取消，再退出）；130 收到 SIGINT 或服务端取消。
  - 非交互（默认）：`Ask` 类问题自动 `Dismiss`、`Approval` 类自动 `Deny`，各打一行 `notice` 说明被拒的工具；`--interactive` 时逐次呈现（P07 前用 mock 断言协议路径）。
  - stderr 只写日志与人读提示；`json` / `stream-json` 模式下 stdout 不被非 JSON 内容污染。
- 不做：
  - 长驻 `gqy stdio` 协议（Q-13-4 待定；如需继承另行开单）。
  - TUI（P05-13）；审批卡片渲染（P07/P09）；会话管理子命令（P09 的 `gqy session`）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `apps/gqy/src/cmd/ask.rs`（新增） | 参数、流程、渲染、退出码 |
| `apps/gqy/src/main.rs`（修改） | 注册子命令 |
| `apps/gqy/tests/ask_contract.rs`（新增） | 契约测试（对 mock 回合） |
| `apps/gqy/tests/fixtures/ask_lines/*.jsonl`（新增） | 只增夹具（与 P05-01 的 `AskLine` 夹具互证） |

## 接口草案

草案，以实现为准。

```text
gqy ask [OPTIONS] [PROMPT]...
  --stdin                      从 stdin 读提示词
  --session <SES> | -c         续接已有会话（-c = 终端集成会话）
  --output-format text|json|stream-json   （默认 text）
  --timeout <DUR>              到点发取消；退出码 124
  --model <NAME>               仅本回合覆盖
  --interactive | --approve=prompt      需要 TTY
```

## 实施步骤

1. 参数与 `text` 流程（无会话 → 建会话 → 提交 → 渲染 → 退出码）。
2. `json` / `stream-json`：接入 `ask_line_from_event`；最后一行保证。
3. 退出码六种路径（含 `--timeout` 与 SIGINT）；非交互问题处理与 `notice`。
4. 契约测试与夹具；`cargo xtask check --fast`、`cargo test -p gqy -p gqy-cli`（apps 的测试目标）；提交：`feat(cli): gqy ask 与 JSONL 契约`。

## 测试与守护

- **契约**（13 §14）：对 mock 回合跑 `stream-json`，逐行断言 `v == 1`、最后一行 `done`；断言未知字段被忽略（旧客户端兼容）；夹具只增。
- **退出码**：成功 0；回合失败 1；用法错误 2；协议不匹配 2；会话不存在 3；`--timeout` 124；SIGINT 130（测试里发信号或注入）。
- **非交互**：Approval 场景自动 Deny 并有一行 `notice`（含工具名）；Ask 场景 Dismiss。
- **stdout 纯净**：`json`/`stream-json` 模式下 stdout 每行都是合法 JSON。
- 先红后绿对照（PR 贴输出）：去掉最后一行保证、把 124 改成 1 各一次。

## 验收流程

```sh
cargo test -p gqy -p gqy-client    # 全绿
cargo xtask check --fast
# 手检（需 daemon 与 mock 供应商）
cargo run -p gqy -- ask --output-format stream-json "你好" | tail -3
cargo run -p gqy -- ask --timeout 2s "写一首很长的诗" ; echo "exit=$?"    # 期望 124（mock 慢流）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 六种退出码与三种输出格式各有断言；最后一行保证有测试
- [ ] `AskLine` 只来自 `gqy-protocol` 的唯一函数（无第二处拼装）
- [ ] 非交互问题处理与 `notice` 有测试；stdout 纯净有测试

## 风险与回退

- **`-c` 终端集成会话**：语义依附 14 的命令表；若 P09 前需要调整，先改 13 §9 与 14 的对应段落。
- **信号与退出码**：SIGINT 处理在 `apps/*` 层允许（库层不装信号；02 §9 的编排由 P05-10 提供）。
- **回退**：`cmd/ask.rs` 是新文件，revert 即可。
