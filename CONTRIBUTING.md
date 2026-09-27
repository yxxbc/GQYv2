# 贡献规范
<!-- GitHub Copilot; updated 2026-09-27T22:46:09Z -->

所有合并到主分支的变更都必须通过 `pr-standards` 和 `workflow-security` 检查。仓库管理员还必须在 GitHub 分支保护规则中将这两项设为必需状态检查，并禁止绕过检查的直接推送；仅添加工作流而不启用必需检查，不构成强制门槛。公开仓库还应将 `dependency-review` 设为必需检查；私有仓库需先启用 GitHub Advanced Security 才能使用该检查。管理员还必须保护 `v*` 版本 tag，禁止更新和删除已发布 tag。

## Commit 与 PR 标题

提交标题和 PR 标题统一使用 Conventional Commits：

```text
<type>(<scope>): <description>
```

`(<scope>)` 可省略；破坏性变更在类型后、冒号前加 `!`。类型仅允许：

`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。

Scope 使用小写 ASCII 字母、数字、点、下划线、斜杠或连字符。描述不能为空，可使用中文或英文。每个提交应表达一个独立、可理解的变更；PR 分支不要包含 merge commit，使用 rebase 整理提交。`pr-standards` 会拒绝包含 merge commit 的 PR 分支。

示例：

```text
feat(agent): 增加会话恢复
fix(storage): 修复 WAL 初始化失败
docs: 补充本地构建说明
feat(api)!: 删除旧版会话接口
```

破坏性变更还应在提交正文中说明迁移影响，并使用 `BREAKING CHANGE:` footer。提交标题带 `!` 时，`pr-standards` 会强制要求非空 footer；迁移影响是否完整仍须由评审确认。PR 标题也必须符合上述格式，以确保 squash merge 生成的提交标题一致。

### 本地自动检查

在仓库根目录运行一次以下命令，为当前 clone 启用 `commit-msg` hook：

```sh
bash scripts/install-hooks.sh
```

此后每次提交都会自动校验提交标题；不符合格式时，Git 会拒绝该次提交。该配置是本地 clone 级别的设置，每个开发者需各自启用。PR 标题、提交范围和变更日志仍由 GitHub Actions 检查。

### 同步本地 worktree

云端 PR 合并到 `main` 后，在仓库任一 worktree 运行：

```sh
bash scripts/sync-worktrees.sh
bash scripts/sync-worktrees.sh --apply
```

第一条命令 fetch `origin` 并报告所有 worktree 的落后、分叉和脏状态，不改本地分支。第二条只会快进干净的 `main`，或 rebase 干净且没有非 main upstream、也没有同名远端分支的本地分支到 `origin/main`。有未提交/未跟踪文件、已配置其他 upstream、upstream 已删除、存在同名远端分支或处于 detached HEAD 的 worktree 会跳过。遇到真实冲突时脚本会 abort 当前 rebase、停止后续同步并返回失败；之前已成功更新的 worktree 不会回滚。

脚本不会删除 worktree、推送分支或 force-push。已发布的 PR 分支需要单独处理历史改写；此脚本不能保证不同分支修改同一文件时绝不冲突。

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

Release Please 根据 Conventional Commits 汇总发布 PR，并同步更新 `version.txt`、`Cargo.toml` 的 `[workspace.package].version`、`CHANGELOG.md` 和 `.release-please-manifest.json`；合并发布 PR 后会创建 `vX.Y.Z` tag 和 GitHub Release notes。初始版本为 `0.1.0`，类别映射见 `release-please-config.json`。版本一致性由 `scripts/check-version-consistency.sh` 校验。

**M1（P05 末）之前不合并发布 PR**：发布 PR 的出现不等于发布，合并才会创建 tag 与 Release；冻结规则与 2026-09-28 的回退记录见 `docs/release-versioning.md`。

为使发布机器人 PR 也触发必需检查，仓库管理员需创建只授权本仓库 `contents`、`issues`、`pull requests` 读写权限的 fine-grained token，并将其保存为 Actions secret `RELEASE_PLEASE_TOKEN`。同时在 Settings → Actions → General 允许 GitHub Actions 创建 pull request。不要将 token 写入文件或日志。

### 依赖与安全审查

- Dependabot 每周检查 GitHub Actions 依赖，并将这些自动更新 PR 标记为 `skip-changelog`。请确认仓库存在 `dependencies` 与 `skip-changelog` 标签；依赖更新若有用户可见影响，维护者应移除豁免并补充 changelog。
- `workflow-security` 使用 zizmor 审查 GitHub Actions 工作流；中等级及以上发现会阻止该检查通过。
- `dependency-review` 在 PR 中阻止引入高危及以上漏洞依赖。仓库 `LICENSE` 尚未确定，当前也未配置依赖许可证允许/禁止清单，因此许可证兼容性还不是硬门禁；在引入外部依赖前必须确定策略并配置检查。该功能适用于公开仓库；私有仓库需要 GitHub Advanced Security 和启用 Dependency graph。
- 私有仓库启用 GHAS 后，还需设置仓库 Actions variable `DEPENDENCY_REVIEW_ENABLED=true` 才会运行 `dependency-review`。
- 仓库管理员应将 `pr-standards`、`workflow-security` 设为必需检查；在 Dependency Review 可用时也将 `dependency-review` 设为必需。另需在 GitHub 仓库设置启用 Dependency graph、Dependabot alerts、secret scanning、私有漏洞报告和分支保护，并为 `v*` tag 配置禁止更新与删除的规则；工作流文件不能代替这些设置。
- Cargo manifest 与 lockfile 尚不存在。加入 Rust 依赖后，应为 Dependabot 增加 Cargo ecosystem，并接入 `cargo audit`/相应供应链检查；在此之前不运行空依赖清单上的 Rust 检查。

## Changelog

项目遵循 Keep a Changelog 的结构和语义化版本原则：

- 每个有面向用户影响的 PR 都必须在 `CHANGELOG.md` 的 `## [Unreleased]` 下新增条目。
- 条目按 `Added`、`Changed`、`Deprecated`、`Removed`、`Fixed`、`Security` 分类；每条内容以 `- ` 开始，说明用户能感知的变化。
- 不在日常 PR 中改写已发布版本的记录。发布时再将 `Unreleased` 内容归入版本号和 `YYYY-MM-DD` 日期标题。
- 仅文档、测试或内部维护且不改变用户可见行为的 PR，可由维护者添加 `skip-changelog` 标签豁免；有用户影响的变更不得使用该标签。

工作流会验证 changelog 是否修改、是否有 `Unreleased` 标题，以及其下是否至少有一个分类条目。标签豁免只跳过 changelog 要求，不跳过提交标题检查。
## 贡献的授权

本项目的源代码采用 [PolyForm Noncommercial 1.0.0](LICENSE) 协议；角色与品牌素材另按 [LICENSE-ASSETS](LICENSE-ASSETS) 授权。提交 PR 即表示你同意：

1. 你有权提交这些内容：它们是你自己写的，或者来源的协议允许这样使用。
2. 你的贡献按本项目的协议发布。
3. 你同时授予项目维护者（yxxbc）一项永久、全球、免费、不可撤销的许可，可以以任何协议（包括商业协议）使用、修改、再授权和发布你的贡献。这样项目以后调整协议时，不用逐个联系贡献者。

你保留自己贡献的版权。
