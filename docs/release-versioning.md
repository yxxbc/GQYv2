# GQY v2 版本与发布流程

本文档说明 GQY v2 当前仓库级版本号和 GitHub 发布流程。仓库现在有一个 Cargo workspace（P00-01），但还没有应用更新器或跨平台制品构建流程；不要照搬其他项目的包版本、平台渠道或资产发布步骤。

## 当前版本来源

Release Please 以仓库根目录为单一发布单元，使用 `simple` release type，并额外同步 Cargo 版本：

| 文件 | 用途 |
| --- | --- |
| `version.txt` | 当前项目版本，Release Please 的版本文件（`simple` 类型读它） |
| `Cargo.toml` | `[workspace.package].version` 是 Rust 侧的版本来源，各 crate 用 `version.workspace = true` 继承；发布 PR 通过 `extra-files` 的 toml 更新器与 `version.txt` 一起更新 |
| `.release-please-manifest.json` | Release Please manifest，根包 `.` 的版本应与 `version.txt` 一致 |
| `release-please-config.json` | 版本递增、tag 格式、changelog 分类与 `extra-files` 规则 |
| `CHANGELOG.md` | `[Unreleased]` 及已发布版本的变更记录 |

当前版本为 `0.2.0`。不要在普通提交或功能 PR 中手动递增 `version.txt`、`Cargo.toml` 版本、manifest 或 Git tag；这些文件由 Release Please 的发布 PR 同步更新。发布 PR 合并后，`Cargo.lock` 里工作区成员的版本会落后一步：下一次 `cargo` 命令（build、metadata 等）会自动刷新，把刷新结果随下一个提交入库即可。

### 为什么不是 `rust` release type（2026-09-28 实测）

`release-please` 的 `rust` 策略会更新每个成员与根 `Cargo.toml` 的 `[package] version` 并写 `Cargo.lock`；但本仓库是**虚拟 workspace**（根 `Cargo.toml` 没有 `[package]`，版本在 `[workspace.package]`），它的 `CargoToml` updater 对没有 `[package]` 的清单会直接抛错（`is not a package manifest (might be a cargo workspace)`，release-please 17.11.2 实测）；成员清单用 `version.workspace = true` 时同样会因 `package.version` 不是字面量而抛错。因此维持 `simple` + `extra-files` 的 toml 更新器（实测格式与注释保留）。跨文件的一致性由 `scripts/check-version-consistency.sh` 守护，接入 CI 见 P00-06。

## 版本格式与递增

版本遵循 `MAJOR.MINOR.PATCH`，Git tag 使用 `v` 前缀：

```text
0.1.0
v0.1.0
```

GQY v2 目前处于 `0.x` 阶段，发布规则与 `release-please-config.json` 保持一致：

- `fix`、`perf` 等兼容修复按 patch 递增。
- `feat` 新功能按 minor 递增，并将 patch 归零。
- `0.x` 阶段的破坏性变更也递增 minor；`bump-minor-pre-major` 已启用。
- 进入 `1.0.0` 后，兼容功能递增 minor，破坏性变更递增 major，兼容修复递增 patch。
- 纯 `docs`、`style`、`test`、`build`、`ci`、`chore` 提交不会单独触发发布；这些类型在 release notes 中隐藏。普通 `refactor` 不应被当成用户功能，只有它有明确兼容性影响时才使用 `!` 并写清迁移说明。

版本表示发布产物，不表示提交次数或日期。一天内的多个 commit/PR 会累计在 `[Unreleased]`，Release Please 将它们汇总到一个发布 PR；若一天内确实发布多次，每个发布都必须合并各自的发布 PR，并使用唯一递增的版本/tag，例如 `0.1.1` 后接 `0.1.2`。日期只写在 changelog 标题中，不拼入版本号。

当前 Release Please 工作流只发布稳定版本，不启用 `alpha`、`beta`、`rc` 或按日期构建的预发布渠道。需要预发布渠道时，应先单独设计分支、版本策略和 GitHub prerelease 标志，不要手动往 `version.txt` 添加后缀。

## Changelog 与 Release Notes

面向用户的 PR 应在 `CHANGELOG.md` 的 `## [Unreleased]` 下记录变化。Release Please 按 `release-please-config.json` 的映射生成分类：

| Commit 类型 | Changelog 分类 |
| --- | --- |
| `feat` | `Added` |
| `fix` | `Fixed` |
| `perf`、`refactor`、`revert` | `Changed` |
| `docs`、`style`、`test`、`build`、`ci`、`chore` | 隐藏，不单独生成发布项 |

发布机器人会创建或更新标题为 `chore: release <version>` 的 PR，并更新 `version.txt`、`Cargo.toml` 的 `[workspace.package].version`、manifest 和 changelog。发布 PR 通过标准检查并合并后，Release Please 创建 `v<version>` Git tag 和 GitHub Release；GitHub Release notes 来自生成的 changelog。PR 标准检查接受 `- ` 和 `* ` 两种列表格式，并允许发布 PR 将内容归档到新版本章节。

仅文档、测试或内部维护且没有用户可见影响的 PR，可由维护者添加 `skip-changelog` 标签；有用户影响的改动不能豁免。

## 自动发布工作流

工作流定义位于 `.github/workflows/release-please.yml`，在 `main` 分支收到 push 后运行。要让机器人创建的发布 PR 也触发 PR 标准与安全检查，仓库管理员必须：

1. 创建仅授权此仓库 `contents`、`issues`、`pull requests` 读写的 fine-grained token。
2. 将 token 保存为 GitHub Actions secret `RELEASE_PLEASE_TOKEN`。不要将 token 写进仓库文件、命令行参数或日志。
3. 在 Settings → Actions → General 允许 GitHub Actions 创建 pull request。
4. 配置分支保护，要求 `pr-standards` 和 `workflow-security` 通过；公开仓库还要求 `dependency-review`。私有仓库需启用 GitHub Advanced Security，并设置 Actions variable `DEPENDENCY_REVIEW_ENABLED=true` 才会运行该检查。

没有 `RELEASE_PLEASE_TOKEN` 时，发布工作流会明确失败，不会回退到无法触发后续 PR 检查的默认 token。Release Please 只在存在可发布的 Conventional Commit 变更时创建发布 PR；只有被隐藏的提交类型时，不会单独发布版本。

## 当前不包含的发布能力

- 有 Cargo workspace 与 `Cargo.lock`（版本由发布 PR 与下一次 `cargo` 命令同步）；仍没有 `cargo audit`、`cargo deny` 门禁与 Rust 构建发布步骤（P16）。
- 当前没有桌面/移动端、CLI 更新器、跨平台打包或自定义 GitHub Release 附件。
- 当前没有稳定/预发布更新通道，也没有按日期命名的开发版。

引入上述能力时，先添加真实的包清单和构建/测试流程，再扩展本指南、Release Please 配置、Dependabot 与 CI；不要在仓库尚无对应实现时记录虚构的构建命令或发布产物。
