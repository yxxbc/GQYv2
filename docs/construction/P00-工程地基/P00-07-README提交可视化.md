<!-- GitHub Copilot; updated 2026-09-27T22:23:08Z -->
<!-- 更新：AI 助手（Cline 会话），2026-09-29 00:22:26 —— 已合入 main（PR #13），状态置为「已完成」；Pages 在线验收完成（部署工作流成功、SVG 可访问）；分支已清理。 -->
# P00-07 · README 提交可视化

| 项 | 值 |
| --- | --- |
| 状态 | 已完成 |
| 依赖 | — |
| 设计依据 | AGENTS.md「施工的规矩」；scripts/CLAUDE.md「规范与职责边界」 |
| 规模 | S（≤1 天） |
| 涉及 crate / 目录 | `README.md`、`README_EN.md`、`.github/workflows/`、`scripts/`、`docs/construction/` |

## 目标

README 同时提供 main 分支提交活跃度日历和 GitHub 原生分支关系图入口。main 上每次 push（含 PR 合并）后，Actions 从完整历史生成 SVG 并发布到 GitHub Pages；不向 main 创建机器人提交。

## 范围

- 做：标准库 Python 日历生成器、临时 Git 仓库测试、main push Pages 工作流、双语 README 链接和施工图登记。
- 不做：功能分支 push 发布、提交活跃度 API、向仓库分支写入生成文件、产品界面图表。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `scripts/generate-commit-activity.py` | 统计 HEAD 可达提交近 365 天的 committer 日期并输出 SVG |
| `scripts/tests/generate-commit-activity.sh` | 在临时 Git 仓库验证每日计数、颜色强度与日期边界 |
| `.github/workflows/commit-activity.yml` | main push 时生成并部署 Pages artifact；权限限定为读取仓库、部署 Pages 和 OIDC |
| `README.md`、`README_EN.md` | 展示日历并链接 GitHub Network 页面 |
| `CHANGELOG.md` | 记录新增可视化入口 |
| `docs/construction/` | 登记本施工单、状态和基础任务图映射 |

## 接口草案

本单不新增应用 API。脚本命令：`python3 scripts/generate-commit-activity.py --repo <path> --output <svg-path>`；`--today YYYY-MM-DD` 仅用于可重复测试。

## 实施步骤

1. 编写本地生成器并以临时仓库覆盖关键日期和强度边界。
2. 配置 main push 工作流部署 SVG，并在中英文 README 增加图表入口。
3. 同步 changelog、施工单状态和施工图基础任务清单。

## 测试与守护

- `bash scripts/tests/generate-commit-activity.sh`：断言同日多提交合并计数、单次提交使用非零强度色、365 天前的提交排除。
- 工作流仅监听 `push` 到 `main`；job 具有 `contents: read`、`pages: write`、`id-token: write`，不使用 secrets 或执行 PR 代码。
- 去掉日期窗口过滤、强度映射或生成 SVG 的实现，对应脚本断言会失败。

## 验收流程

1. 本地运行 `bash scripts/tests/generate-commit-activity.sh`，期望退出码为 0。
2. 在仓库 Settings → Pages 中将 Build and deployment 的 Source 设为 GitHub Actions。
3. 将本分支合并到 main 后，检查 `Commit activity` 工作流成功，随后确认 `https://yxxbc.github.io/GQYv2/commit-activity.svg` 可打开。
4. 确认 README 中日历可加载，Network 链接可打开并显示分支提交关系。

## 完成判据

- [x] 生成器测试覆盖计数、强度和一年窗口边界。
- [x] 工作流只在 main push 触发，并采用最小 job 权限与 SHA 固定的 Actions。
- [x] 中英文 README、`[Unreleased]` 与施工图已同步。
- [x] 仓库 Pages 来源设为 GitHub Actions，合并后完成在线验收（部署工作流成功、SVG 可访问）。

## 风险与回退

- GitHub Pages 未启用时部署会失败；管理员完成验收流程第 2 步后再触发一次 main push 即可恢复。
- 回退本单时删除工作流、生成器、测试及 README 图表入口，不影响 Git 历史和产品代码。