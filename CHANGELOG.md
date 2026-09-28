# Changelog
<!-- GitHub Copilot; updated 2026-09-27T22:51:04Z -->

本项目的变更记录遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [0.4.0](https://github.com/yxxbc/GQYv2/compare/v0.3.0...v0.4.0) (2026-09-28)


### Added

* **lints:** 落地体积门禁、clippy 全量 lints 与 cargo xtask check ([#28](https://github.com/yxxbc/GQYv2/issues/28)) ([5d29aab](https://github.com/yxxbc/GQYv2/commit/5d29aabdb0c16708e14dda9df661e3ef86dc8d06))
* **pr-checks:** 增加对大提交的检查，允许通过标签豁免体量限制 ([#24](https://github.com/yxxbc/GQYv2/issues/24)) ([2355bd3](https://github.com/yxxbc/GQYv2/commit/2355bd3abadc35e5c10fe315bb419dd25f2cd174))

## [0.3.0](https://github.com/yxxbc/GQYv2/compare/v0.2.0...v0.3.0) (2026-09-27)


### Added

* **scripts:** add guarded worktree sync ([#18](https://github.com/yxxbc/GQYv2/issues/18)) ([1918151](https://github.com/yxxbc/GQYv2/commit/19181515fbda1f6406fb4dd8b5dd7d510947cc9b))

## [0.2.0](https://github.com/yxxbc/GQYv2/compare/v0.1.0...v0.2.0) (2026-09-27)


### Added

* **readme:** add auto-updating commit visualizations ([#13](https://github.com/yxxbc/GQYv2/issues/13)) ([e898d4d](https://github.com/yxxbc/GQYv2/commit/e898d4d34efc7f82f27c4e8932e786a3af460187))
* **workspace:** 搭建 Cargo workspace 与 17 个库 crate 骨架 ([#6](https://github.com/yxxbc/GQYv2/issues/6)) ([73b247a](https://github.com/yxxbc/GQYv2/commit/73b247ab7a79f0c187eb94f9513af267b5d6e8b0))


### Fixed

* **scripts:** changelog 判定兼容版本段在 [Unreleased] 之前 ([#10](https://github.com/yxxbc/GQYv2/issues/10)) ([061060f](https://github.com/yxxbc/GQYv2/commit/061060f51ba8d945db5b0998cef4a819797c61cb))

## [Unreleased]

### Added

* **workspace:** 搭建 Cargo workspace 与 17 个库 crate 骨架（[#6](https://github.com/yxxbc/GQYv2/issues/6)，[73b247a](https://github.com/yxxbc/GQYv2/commit/73b247ab7a79f0c187eb94f9513af267b5d6e8b0)）
* **readme:** add auto-updating commit visualizations（[#13](https://github.com/yxxbc/GQYv2/issues/13)，[e898d4d](https://github.com/yxxbc/GQYv2/commit/e898d4d34efc7f82f27c4e8932e786a3af460187)）
* Add an automatically updated commit activity chart and branch graph link to the READMEs.
- Add a guarded script to report and sync local worktrees after main advances.

### Fixed

* **scripts:** changelog 判定兼容版本段在 [Unreleased] 之前（[#10](https://github.com/yxxbc/GQYv2/issues/10)，[061060f](https://github.com/yxxbc/GQYv2/commit/061060f51ba8d945db5b0998cef4a819797c61cb)）
