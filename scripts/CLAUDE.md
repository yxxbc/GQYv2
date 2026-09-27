# 仓库级 AI 工程护栏

本文件是整个仓库的 AI 工作规范和本地快速检查工具索引，不限于 `scripts/` 目录。根目录 `CLAUDE.md` 会导入本文件；处理代码、文档、资源、测试、GitHub Actions 或工程配置前，均应遵守这里的要求。

## 工作原则

- 先阅读相关实现、调用方和仓库约定，定位根因后再改；变更保持最小、模块化，不创建上帝文件或顺手重构无关内容。
- 不确定会影响接口、架构、兼容性、数据或安全边界时，说明根因、推荐方案和取舍，请用户决定后再扩大范围。
- 以仓库实际状态为准。文档中尚未实现的 Rust、Web、TUI、数据库或 Agent 设计只能描述为规划，不得写成现成功能；没有 `Cargo.toml` 时不要声称 `cargo test` 可运行。
- 保留用户已有改动；不擅自清理、覆盖、提交或重写历史。不得将密钥、凭据或个人环境配置写入仓库。

## 规范与职责边界

- 本文件维护仓库级 AI 规则和本地护栏约定。新增或调整仓库级检查、辅助脚本、脚本专属配置和测试夹具，优先放在 `scripts/`，按职责拆分并提供清楚的用法。
- `scripts/` 是检查逻辑与本地工具的位置，不是所有文档或配置的收纳目录：贡献者说明留在 `CONTRIBUTING.md`，产品/技术说明留在 `README*` 和 `docs/`，GitHub Actions 工作流必须留在 `.github/workflows/` 并调用 `scripts/` 中可本地运行的检查。
- 同一规范只保留一个可执行的事实来源。文档负责说明规则，CI 和本地工具负责验证；改动规则时同步更新受影响的文档、检查脚本和工作流调用参数。
- 工作流遵循最小权限，不暴露 secrets，不在不可信 PR 代码上使用高权限触发器。仓库规则、必需状态检查和绕过权限等 GitHub 端设置，不能假称已由本地 YAML 自动完成。
- 用户可见改动按 Keep a Changelog 写入 `CHANGELOG.md` 的 `[Unreleased]`；纯内部维护无需制造用户可见条目。验收流程完成后再按用户要求处理发布记录，不自行 commit。

## 当前提交与 PR 护栏

- `scripts/check-commit-subject.sh` 是 Conventional Commits 标题规则的唯一实现。其他脚本调用它，不复制正则。
- `scripts/check-pr-standards.sh` 接收 `<base-sha> <head-sha> <pr-title> <skip-changelog-label> <large-commit-approved-label>`，检查 PR 标题、逐提交标题、体量、merge commit、破坏性 footer 与 changelog；单个提交最多 10 个文件、增删合计最多 500 行。超限仅能由维护者通过 `large-commit-approved` PR 标签豁免。调整参数或语义时同步检查 `.github/workflows/pr-standards.yml` 和 `CONTRIBUTING.md`。
- `.githooks/commit-msg` 调用共享标题校验器；`scripts/install-hooks.sh` 只设置当前 clone 的 `core.hooksPath`，不得改全局 Git 配置。
- `skip-changelog` 只能豁免 changelog 要求，不能跳过标题校验。PR 标题、分支提交范围和标签判断由 CI 验证；本地 hook 不应伪装成完整 PR 检查。
- 版本策略以 `docs/release-versioning.md` 为详细规范、`CONTRIBUTING.md` 为贡献者摘要：版本代表发布产物，不按提交/修复次数 bump；一天多次发布时，每个发布递增到唯一版本。添加包清单或本地版本检查器时，保证包版本、Git tag 与 changelog 标题一致。
- Dependabot 更新规则在 `.github/dependabot.yml`；Release Please 使用 `version.txt`、`.release-please-manifest.json` 和 `release-please-config.json`，并通过 `extra-files` 的 toml 更新器同步 `Cargo.toml` 的 `[workspace.package].version`（改动这条链路时同步 `docs/release-versioning.md` 与 `scripts/check-version-consistency.sh`）；不得手工复制版本/changelog 生成逻辑。
- `.github/workflows/security-review.yml` 负责 zizmor 与 dependency review。Cargo 清单未出现前不添加会因缺少 lockfile 而失败的 Rust 审计步骤。

## 脚本与本地工具约定

- Shell 脚本使用 Bash 和 `set -euo pipefail`；引用变量，避免 `eval`、来源不明的代码及 Bash 4+ 专属语法，以兼容 macOS Bash 3.2 和常见 Linux Bash。
- 脚本应可从仓库外的当前目录可靠定位仓库/自身路径；检查 Git 元数据和依赖是否存在，并给出明确错误与用法。
- 本地快速检查应与 CI 使用同一实现，且尽量只读、可重复、无需秘密或外部服务。会修改 Git 配置、文件或环境的工具必须明确范围，默认不得产生全局副作用。
- 保留 hook、安装器等需要的可执行权限；测试 Git 行为时使用临时仓库，不更改用户 clone 的配置或提交历史。
- 脚本测试放 `scripts/tests/`：一个被测脚本一个同名测试，用 `bash scripts/tests/<脚本名>.sh` 运行（可选第一个参数传被测脚本路径，用于红绿对照）；用例全部在 `mktemp -d` 的临时 git 仓库里做，不读网络、不需要 secrets。改动校验脚本时，先让对应测试覆盖到新规则，并用旧版脚本跑一遍确认会变红。

## 验证要求

- 先运行能最快证伪本次改动假设的窄检查，再按影响范围补充测试；不要用宽泛测试掩盖局部问题。
- 修改 Shell 时至少运行 `bash -n`。修改校验器时测试有效和无效样例；修改 hook/安装器时在临时 Git 仓库验证；修改 PR 检查时验证提交范围、提交体量边界、批准标签、`Unreleased` 条目、`skip-changelog` 豁免和工作流参数。
- 修改工作流时核实触发器、权限、输入/输出以及本地脚本调用保持一致；修改文档时核对链接和描述与实际实现一致。
- 结束时简要说明改了什么、实际跑了哪些验证、哪些仓库外设置仍需用户或管理员配置，并给出可照做的验收步骤。


你可以动态调整此文档