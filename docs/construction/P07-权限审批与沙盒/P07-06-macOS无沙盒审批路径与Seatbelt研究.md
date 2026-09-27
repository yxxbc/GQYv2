# P07-06 · macOS 无沙盒审批路径与 Seatbelt 研究

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P07-04 |
| 设计依据 | designs/08-权限审批与沙盒.md §6.4（现状、路径守卫、Seatbelt 研究清单 ①–⑤）、§4 第 3 步与§4.1（无后端平台规则全文）、§5.1（卡片字段）、§7（场所信任映射）、§8（边界）、§9.2（事件）、§13（D-02/Q-08-2/Q-08-3）；designs/00-设计理念.md §5（「无沙盒后端时的 Exec」已定决策）；designs/09-感知矩阵.md §4.1（`SandboxFs = Missing`、能力描述进模型）、§6（`gqy doctor`）；designs/02-运行时与并发模型.md §8（授权按请求查询）；designs/13-网关与API协议.md §8.1（无沙盒执行不在本地令牌防护范围内的边界声明） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-sys`（`sandbox/probe.rs` 的 macOS 产出与错误形态）、`crates/gqy-engine`（`approval.rs` 的 unsandboxed 卡片字段与自动转沙盒联测）、`crates/gqy-tools`（`permission.rs` 的 `unsandboxed:` 键路径联测）、`docs/designs/08-权限审批与沙盒.md`（§6.4 研究结论回写） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

macOS 上「无后端」路径完整可用且被测试钉住：Owner `Workspace` Exec → 无沙盒 + 每次审批（卡片标注 unsandboxed、可「本会话同意」）；Member / External 拒绝；`Full` 宿主直跑；后端一旦可用（探测结果变化）自动转沙盒内执行，无需用户操作。Seatbelt 按 08 §6.4 的 ①–⑤ 清单研究并写回设计、等用户拍板——本单**不实现** Seatbelt 后端。

## 范围

- 做：
  - 探测产出：macOS 上 `SandboxFs = Missing{reason: "no sandbox backend on macos"}`（平台分支只在 `gqy-sys` 内；业务 crate 不变）；`SandboxView` 的 unsandboxed 呈现字段（审批卡片与 doctor 读取接口共用）。
  - 端到端 unsandboxed 路径：`Ask`（键带 `unsandboxed:`）→ AllowOnce 执行本次 / AllowForSession 写授权 / Deny；沙盒内得到的会话授权**不**命中无沙盒执行（08 §8 的行）；Member / External 失败关闭且未 spawn。
  - 自动转沙盒：可用性按请求查询（02 §8 口径），探测变 Available/Degraded → 同会话下一次 Exec 在沙盒内（用注入的假探测联测，不需要真的装后端）。
  - 边界声明与守卫：路径守卫在 macOS 照常生效（P07-04 的实现）；`openat2` 缺失走回退并标注 TOCTOU；审批卡片与 doctor 读取接口写明「无沙盒执行等价于 Owner 本人在终端执行」。
  - Seatbelt 研究（D-02）：① 当前 macOS 版本上 `sandbox-exec -p` 可用性与弃用警告的实际影响；② 对 `/dev/tty`、PTY、`xcrun` 工具链的兼容性；③ `(deny network*)` 的能力与两平台差异（Landlock 没有）；④ 失败错误形态与探测方法；⑤ 与 App Sandbox（桌面宿主签名分发）的关系。结论写成报告并并入 08 §6.4（配置草案必须包含拒绝读取 `run/`、`config/` 的硬规则）。
- 不做：
  - Seatbelt 后端实现（D-02 拍板后才开单）；网络限制的启用（Q-08-3 第一版不限制）。
  - Landlock（P07-05）、Windows（08 §6.5 维持一律拒绝，不在本单）；`gqy doctor` CLI 本体（P11-05）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-sys/src/sandbox/probe.rs`（修改） | macOS 产出 `Missing{reason}`；错误形态与探测方法记录（供研究报告引用） |
| `crates/gqy-engine/src/approval.rs`（修改） | unsandboxed 卡片字段、「本会话同意」与自动转沙盒联测 |
| `crates/gqy-tools/src/permission.rs`（修改） | 注入可用性的 `unsandboxed:` 键路径联测（不改平台分支） |
| `crates/gqy-engine/tests/unsandboxed.rs`（新增） | 见「测试与守护」 |
| `docs/designs/08-权限审批与沙盒.md`（修改） | §6.4 研究结论与建议（用户确认后定稿） |

## 接口草案

草案，以实现为准。

```rust
// gqy-sys/src/sandbox/probe.rs（平台分支只在 gqy-sys 内；业务 crate 问能力、不问平台）
pub fn probe() -> CapState;
// macOS → Missing { reason: "no sandbox backend on macos",
//                   effect: "commands need approval and run unsandboxed" }

// 联测用：注入可用性的测试替身（不新增业务类型；命名含 Test，文档标注）
pub struct TestAvailability(pub SandboxAvailability);

// 研究骨架（不是产品接口）：sandbox-exec 配置文件草案随报告提交
// (version 1)(deny default)(allow process-exec process-fork)
// (allow file-read* (subpath "/usr") …)(allow file-write* (subpath "<workspace>"))
// 并包含 08 §6.2 硬规则：拒绝读取 <data>/run 与 <data>/config
```

## 实施步骤

1. 用注入的 `Unavailable` 在 Linux CI 上先写整条 unsandboxed 审批路径的会红用例（P07-01/02 的联测）。
2. macOS 探测产出与 `SandboxView`/doctor 读取接口文案；卡片醒目标注字段。
3. 自动转沙盒联测（假探测翻转），确认不依赖用户操作。
4. macOS 实机跑守卫与命令路径验收（需用户环境）；CI 的 macOS runner 跑单元侧与注入侧。
5. Seatbelt 研究实验（需用户环境）：按 ①–⑤ 逐条记录、脚本随 PR；结论回写 08 §6.4；等用户确认。
6. `cargo xtask check --fast`、`cargo test -p gqy-sys -p gqy-engine -p gqy-tools`；提交：`feat(perm): macOS 无沙盒审批路径与 Seatbelt 研究`。

## 测试与守护

- **失败关闭**：Member / External 在 `Unavailable` 下 Exec → `Deny{layer: Sandbox}` 且 spawn 计数器为 0；去掉第 3 步此测试红。
- **unsandboxed 键**：Owner `Workspace` → `Ask` 且键带 `unsandboxed:`；沙盒内授权不命中（08 §8 的行）；AllowForSession 后同键不再提问；Dangerous 仍提问。
- **自动转沙盒**：可用性翻转为 `Available` → 同会话下一次 Exec 走沙盒路径（spawn 侧记录断言），审批行为随之变化。
- **`Full` 不读 `[exec].sandbox`**：宿主直跑、只有 Dangerous 审批。
- **研究交付**：08 §6.4 的 ①–⑤ 每条有结论，或如实写「未能验证 + 原因」；实验脚本可复跑。
- 先红后绿对照（PR 贴输出）：把可用性换回恒 `Available`、删 Member 拒绝各一次。

## 验收流程

```sh
cargo test -p gqy-sys -p gqy-engine -p gqy-tools   # 全绿
cargo xtask check --fast
# macOS 实机（需用户环境）：临时数据目录起 daemon，mock 触发 run_command →
#   卡片标注 unsandboxed；AllowOnce 后命令在宿主真的跑了；touch /etc/x 仍被系统权限拒
# Seatbelt 研究（需用户环境）：按报告脚本逐条跑 ①–⑤；结论并入 08 §6.4 待用户拍板
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] unsandboxed 三态（Owner / Member / External）在注入与 macOS 实机上都成立
- [ ] 自动转沙盒联测通过，不依赖用户操作
- [ ] Seatbelt ①–⑤ 结论写回 08 §6.4 并经用户确认；未确认期间不实现后端
- [ ] TOCTOU 标注与「等价于 Owner 本人在终端执行」的边界声明在 doctor 读取接口可见（文案随 P11-05）

## 风险与回退

- **研究不落地**：结论可能是「不采用 Seatbelt」——这也算完成；不许顺手实现半套后端。
- **实机差异**：macOS 验收依赖用户环境；注入版本保证 CI 可重复，两者口径写进 PR。
- **回退**：探测文案与联测随单 revert；08 §6.4 的修改留在文档历史，不影响代码路径。
