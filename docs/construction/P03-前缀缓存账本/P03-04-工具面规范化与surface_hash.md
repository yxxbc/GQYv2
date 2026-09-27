# P03-04 · 工具面规范化与 surface_hash

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P03-02 |
| 设计依据 | designs/04-前缀缓存账本.md §8（工具面规范化的七条）、§13（形状夹具）；designs/07-工具系统.md §3.1（ToolSpec）、§6（LoadingMode）、§12.1（形状夹具）；designs/06 §6.2（规范 JSON） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-ledger`（`surface.rs`）、`crates/gqy-core`（`StableSection` 若需补字段） |

## 目标

工具面满足字节契约的全部性质：**按名排序、逐工具规范 JSON、`surface_hash = blake3(canonical_json(sorted_tool_specs))`、会话内恒定**，并给出形状夹具的格式与校验函数（供 P06 的 `gqy-tools` 直接使用）。做完之后，`StableSection` 有了稳定的 tools 字节与哈希，`[S]` 段才真正"纪元内字节恒定"。

## 范围

- 做：
  - `normalize(specs: &[ToolSpec]) -> NormalizedSurface { bytes: Vec<u8>, surface_hash: [u8; 32] }`：
    - 按 `name` 字节序排序（与注册顺序无关；乱序输入相同输出）；
    - 逐工具 `gqy_core::canonical_json`（04 §8.2）；不含协议外壳（适配器的事，06 §6）；
    - `surface_hash` 取 32 字节；展示前 16 hex 的辅助函数。
  - stub 形态校验：`LoadingMode::Stub` 的 `description ≤ 60` 字符与固定参数壳 `{"type":"object","properties":{"arguments":{"type":"object"}}}`（04 §8.3）——本单提供校验函数（生成在 P06）。
  - `build_stable_section(specs, system_text, wire_prompt_hash, ...) -> StableSection`（04 §9.1 的字段：`surface_hash`、`wire_prompt_hash`、`layout_version` 等）；system 文本本单只透传（组装在 P11-06）。
  - 形状夹具的**格式与校验**：`crates/gqy-tools/tests/fixtures/shapes/<surface>.json` 的 schema（工具逐个 `sha256(canonical_json(spec))` + `surface_hash`）与 `verify_shape_fixture()`（P06 生成夹具时调用；本单先把格式钉住，避免两处定义）。
  - `surface_changed(prev: &NormalizedSurface, next: &NormalizedSurface) -> bool`（供纪元开启判定；真正的开纪元在 P03-05 与 P04）。
- 不做：
  - 工具注册表、stub 生成、`load_tools`、Hybrid 合并（P06；04 §8.5–6 的"每回合至多一次变更"在 P06/P04 落地）。
  - 真实 system 文本组装（P11-06）。
  - 门禁里对 fixtures 的比对（P03-06 的 `cargo xtask fixtures refresh` 负责重写；本单只定格式与校验器）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-ledger/src/surface.rs`（新增） | `normalize`、stub 校验、`build_stable_section`、`surface_changed`、夹具校验器 |
| `crates/gqy-ledger/tests/surface.rs`（新增） | 见“测试与守护” |
| `crates/gqy-ledger/tests/fixtures/shapes/example.json`（新增） | 夹具格式样例（真实 surface 由 P06 生成） |

## 接口草案

草案，以实现为准。

```rust
// gqy-ledger/src/surface.rs
pub struct NormalizedSurface { pub bytes: Vec<u8>, pub surface_hash: [u8; 32] }

/// 04 §8：排序 + 逐工具规范 JSON + 哈希。纯函数。
pub fn normalize(specs: &[ToolSpec]) -> Result<NormalizedSurface, SurfaceError>;
pub fn validate_stub_form(spec: &ToolSpec) -> Result<(), SurfaceError>;
pub fn build_stable_section(surface: &NormalizedSurface, system_text: &str, meta: StableMeta) -> StableSection;
pub fn surface_changed(a: &NormalizedSurface, b: &NormalizedSurface) -> bool;

/// 形状夹具：逐工具 sha256 + surface_hash；P06 生成，CI 比对。
pub fn verify_shape_fixture(specs: &[ToolSpec], fixture_json: &str) -> Result<(), ShapeFixtureError>;
```

## 实施步骤

1. `normalize` 与排序测试（乱序/重复名；重复名报错）。
2. stub 校验规则与用例（>60 字符、壳被改）。
3. `build_stable_section` 字段与 `surface_changed`；`StableSection` 条目写入在 P03-03 的 `open_epoch` 里已有路径。
4. 夹具格式样例 + 校验器；一个"改一个字节 → 校验器报第一个不同工具的漂移"的用例。
5. `cargo xtask check --fast`、`cargo test -p gqy-ledger`；提交：`feat(ledger): 工具面规范化与 surface_hash`。

## 测试与守护

- **排序**：打乱输入、含非 ASCII 名的排序（字节序，不是 locale 序）；重复名报错。
- **规范 JSON**：同一 spec 两次编码相同；非 ASCII 原样；键序稳定（复用 P01-01 的写出器断言）。
- **哈希**：`surface_hash` 与手算 blake3 一致（一个固定向量）；改任一 spec 一个字节 → 哈希变化。
- **stub**：60 字符边界；参数壳被改 → 报错；`Full`/`Hybrid` 形态不被 stub 校验拦截。
- **夹具**：样例通过校验；篡改某工具的 sha256 → 报错且指出工具名。
- 先红后绿对照（PR 贴输出）：去掉排序（按输入序）→ 排序用例红。

## 验收流程

```sh
cargo test -p gqy-ledger           # 全绿
cargo xtask check --fast           # 全绿
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 04 §8 的第 1、2、4 条逐条有测试；stub 形态（第 3 条）有校验
- [ ] `StableSection` 字段与 04 §9.1 一致；`surface_hash` 有固定向量
- [ ] 形状夹具格式与校验器就位（P06 直接复用，两处没有第二份定义的余地）

## 风险与回退

- **与 P06 的边界**：本单不生成 stub、不管注册表；P06 生成后把真实 surface 写进 `tests/fixtures/shapes/` 时应只调用本单的校验器。
- **`StableMeta` 的字段**：`wire_prompt_hash` 的算法在 P11-06 定；本单只透传与存储。
- **回退**：新增模块，revert 即可。
