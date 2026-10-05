# 贡献规范
<!-- GitHub Copilot; updated 2026-09-27T22:46:09Z -->
<!-- 更新：AI 助手（Cline 会话），2026-09-29 00:22:26 —— 补 CI 作业列表与必需检查管理员清单，修正供应链审查的过时说明。 -->
<!-- 更新：AI 助手（Cline 会话），2026-10-05 —— 对齐现状：三平台各跑 `cargo xtask check` + Linux 长跑、分支跑绿后快进合 main（施工方案第一节）；删掉已经不存在的 `scripts/ci-*.sh`、`deny.toml`、worktree 同步脚本那一套；许可证改成 GPL-3.0-or-later。 -->

这个仓库按 `docs/construction/README.md`（施工方案）干活：一步一张施工单，改动在 `step/<编号>-<名字>` 分支上做，CI（`.github/workflows/ci.yml`）在 Linux、macOS、Windows 上各跑一遍 `cargo xtask check`，另有一项 Linux 长跑（随机测试两万例）；全绿再快进合进 `main`。真开的 PR（发布 PR、依赖更新）另过 `pr-standards` 与 `workflow-security` 两道检查。`v*` 版本 tag 由 GitHub 规则保护，禁止更新和删除已发布 tag。

## Commit 与 PR 标题

提交标题和 PR 标题统一使用 Conventional Commits：

```text
<type>(<scope>): <description>
```

`(<scope>)` 可省略；破坏性变更在类型后、冒号前加 `!`。类型仅允许：

`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。

Scope 使用小写 ASCII 字母、数字、点、下划线、斜杠或连字符。描述不能为空，可使用中文或英文。每个提交应表达一个独立、可理解的变更；PR 分支不要包含 merge commit，使用 rebase 整理提交。单个提交最多修改 10 个文件、最多增删 500 行；超过任一上限必须拆分提交。`pr-standards` 会拒绝超限提交和包含 merge commit 的 PR 分支。

示例：

```text
feat(agent): 增加会话恢复
fix(storage): 修复 WAL 初始化失败
docs: 补充本地构建说明
feat(api)!: 删除旧版会话接口
```

破坏性变更还应在提交正文中说明迁移影响，并使用 `BREAKING CHANGE:` footer。提交标题带 `!` 时，`pr-standards` 会强制要求非空 footer；迁移影响是否完整仍须由评审确认。只有确实无法拆分的单一变更，才可由维护者添加 `large-commit-approved` 标签豁免体量门禁；PR 描述必须解释原因，标签不能作为常规绕过方式。二进制文件计入文件数，不计入增删行数。PR 标题也必须符合上述格式，以确保 squash merge 生成的提交标题一致。

### 本地自动检查

在仓库根目录运行一次以下命令，为当前 clone 启用 `commit-msg` hook：

```sh
bash scripts/install-hooks.sh
```

此后每次提交都会自动校验提交标题；不符合格式时，Git 会拒绝该次提交。该配置是本地 clone 级别的设置，每个开发者需各自启用。提交体量、PR 标题、提交范围和变更日志由 GitHub Actions 检查。

## CI 检查

推 `main`（或任何 `step/**` 分支）都会跑 `.github/workflows/ci.yml`，本地跟 CI 是同一条命令：

| 作业 | 内容 | 本地复现 |
| --- | --- | --- |
| `check`（ubuntu-24.04 / macos-latest / windows-latest） | 三台机器各跑一遍门禁：格式、clippy、文档、分层、纯逻辑、行数、许可证、测试 | `cargo xtask check` |
| `randomized`（ubuntu-24.04，release） | 长跑：随机测试接着平时的往后跑两万例（平时的门禁跳过标 `#[ignore]` 的这几个） | `cargo test --release --workspace -- --ignored` |

为什么是三台机器各跑一遍：`docs/construction/0-3-三平台CI.md`。工作流顶层 `permissions: {}`，两个作业只授予 `contents: read`；checkout 不把凭据留在 `.git/config`（`persist-credentials: false`），第三方 Action 固定到完整 commit SHA。

管理员可把这些检查配成必需（现在没配：`main` 的现行流程是分支跑绿后快进推上去，见施工方案第一节）；`v*` tag 的规则（禁止更新与删除）已经配着。工作流文件不能代替这些设置。

## 版本号与发布节奏

完整版本基线、Release Please 的变更分类和发布操作见[版本与发布流程](docs/release-versioning.md)。

版本号遵循 `MAJOR.MINOR.PATCH`。版本号代表一个发布产物，不代表提交次数或日期：日常提交和 PR 不 bump 版本，先累计在 `CHANGELOG.md` 的 `[Unreleased]` 中；每次实际发布时，再根据这一批变更的最高影响级别确定一次版本递增。

当前项目处于 `0.x` 阶段：

- `PATCH`：向后兼容的 bug、安全或性能修复，例如 `0.6.0` → `0.6.1`。
- `MINOR`：新增功能，或任何可能不兼容的 API/行为变更，例如 `0.6.1` → `0.7.0`；同时将 patch 归零。
- 仍处于 `0.x` 时不递增 major。发布 `1.0.0` 表示承诺稳定的公共接口。

从 `1.0.0` 起使用标准 SemVer：兼容的新功能递增 minor 并将 patch 归零；不兼容变更递增 major；兼容修复递增 patch。

一天内可以多次发布修复，每个已发布产物必须使用唯一、递增的版本号，例如 `0.6.1`、`0.6.2`。同一天的多次提交或修复若尚未发布，不各自占用版本号；日期只记录在 changelog 发布标题中，不拼入版本号。预发布使用 SemVer 后缀，例如 `0.7.0-alpha.1`、`0.7.0-beta.1`、`0.7.0-rc.1`；正式版使用对应的无后缀版本 `0.7.0`。

每次发布都应将 `[Unreleased]` 内容归入对应的 `## [版本号] - YYYY-MM-DD`，并创建匹配的不可变 Git tag `v版本号`。仓库管理员必须在 GitHub Rulesets 中保护 `v*`，禁止更新与删除 tag。未来增加 `Cargo.toml` 或其他包清单后，其中的版本字段必须与发布 tag 和 changelog 标题一致；不要在尚无版本字段的当前脚手架中伪造版本元数据。

### 自动发布

Release Please 根据 Conventional Commits 汇总发布 PR，并同步更新 `version.txt`、`Cargo.toml` 的 `[workspace.package].version`、`CHANGELOG.md` 和 `.release-please-manifest.json`；合并发布 PR 后会创建 `vX.Y.Z` tag 和 GitHub Release notes。类别映射见 `release-please-config.json`；这条链路没有门禁守（改的时候自己对一遍三处，见 `docs/release-versioning.md`）。

**发布 PR 的出现不等于发布**：合并它才会创建 tag 与 Release；合不合由项目主人定（发布冻结的来历与 2026-09-28 的回退记录见 `docs/release-versioning.md`）。

为使发布机器人 PR 也触发必需检查，仓库管理员需创建只授权本仓库 `contents`、`issues`、`pull requests` 读写权限的 fine-grained token，并将其保存为 Actions secret `RELEASE_PLEASE_TOKEN`。同时在 Settings → Actions → General 允许 GitHub Actions 创建 pull request。不要将 token 写入文件或日志。

### 依赖与安全审查

- Dependabot 每周检查 GitHub Actions 依赖，并将这些自动更新 PR 标记为 `skip-changelog`。请确认仓库存在 `dependencies` 与 `skip-changelog` 标签；依赖更新若有用户可见影响，维护者应移除豁免并补充 changelog。
- `workflow-security` 使用 zizmor 审查 GitHub Actions 工作流；中等级及以上发现会阻止该检查通过。
- `dependency-review` 在 PR 中阻止引入高危及以上漏洞依赖（公开仓库可用；私有仓库要先开 GitHub Advanced Security 和 Dependency graph，并设 Actions variable `DEPENDENCY_REVIEW_ENABLED=true`）。
- 许可证兼容不是 `dependency-review` 管的：本仓库是 GPL-3.0-or-later，引入依赖之前先看它的许可证能不能和本仓库合在一起发——门禁的「许可证」一项查，规矩和名单见蓝图 `docs/blueprint/licenses.md`。
- 管理员可把 `pr-standards`、`workflow-security` 设为必需检查（现在没配，见「CI 检查」那一节），`v*` tag 的规则（禁止更新与删除）已经配着。工作流文件不能代替这些设置。

## Changelog

发布说明由 Release Please 按提交类型自动生成（`feat` → Added、`fix` → Fixed，`docs`/`ci`/`chore` 这类隐藏；见 `release-please-config.json`）；已发布的版本节不改写。

- 直接推 `main` 的提交不强制改 `CHANGELOG.md`：合发布 PR 时机器人按提交生成。
- 走 PR 的（发布 PR、依赖更新、别人提的 PR）由 `pr-standards` 检查有没有 `CHANGELOG.md` 的改动与 `## [Unreleased]` 标题、其下有没有分类条目；纯文档/内部维护的 PR 由维护者加 `skip-changelog` 标签豁免（有用户影响的变更不得用这个标签）。
- 标签只跳过 changelog 的要求，不跳过提交标题检查。

## 贡献的授权

本项目的源代码采用 [GPL-3.0-or-later](LICENSE) 协议；角色与品牌素材另按 [LICENSE-ASSETS](LICENSE-ASSETS) 授权。提交 PR 即表示你同意：

1. 你有权提交这些内容：它们是你自己写的，或者来源的协议允许这样使用。
2. 你的贡献按本项目的协议发布。
3. 你同时授予项目维护者（yxxbc）一项永久、全球、免费、不可撤销的许可，可以以任何协议（包括商业协议）使用、修改、再授权和发布你的贡献。这样项目以后调整协议时，不用逐个联系贡献者。

你保留自己贡献的版权。
