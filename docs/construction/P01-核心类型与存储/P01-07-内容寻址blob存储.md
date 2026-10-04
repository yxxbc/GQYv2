# P01-07 · 内容寻址 blob 存储

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P01-04（文件侧）；`blobs` 表在 P01-06 的 `0001` 里（登记行的联动用例在 0001 合入后补） |
| 设计依据 | designs/10-存储与数据演进.md §6（布局、写入顺序、读取校验、回收接缝）、§4.4；designs/19-可观测性与测试.md §4；gqy-agent-remake 施工单 3-3（对照参考） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-store` |

## 目标

字节级内容寻址存储：`blake3` 哈希、`<前两位>/<次两位>/<64 hex>` 路径、临时文件写完才 `rename`、已有内容去重不重写、读取时校验哈希。做完之后，账本的大载荷（P03）、工具输出（P06）、附件（P07）都有可用的落地仓库；“先落 blob，再提交引用行”的顺序有一条经过测试的路径。

## 范围

- 做：
  - `BlobStore`：`new(blobs_dir)`、`put(bytes)`（流式写 `tmp/<ulid>`、边写边算哈希、`sync_data`、按 10 §6.2 的四步处理已有/不存在、必要时 `rename` + 同步父目录）、`get(hash)`（校验哈希，`BlobCorrupt { hash, path }` 时不改文件）、`path_for(hash)`（给 GC/备份用）。
  - `durable` 助手（从 gqy 3-3 的 `durable.rs` 取经）：`sync_dir`、逐层建目录并同步父目录（新建的每一层）；P01-08 与日志/备份路径复用。
  - 测试：往返（含二进制）、去重、崩溃残留、校验失败、并发同名（两个写者同内容）。
- 不做：
  - GC（标记—清除、宽限期）与 `gc_runs`（10 §6.3；随后台作业 P13-02 落地；本单只留 `path_for` 与 mtime 语义的接缝）。
  - 行内/`blob_inline_max` 的判定（调用侧决策；P03 起使用 `StoreConfig.blob_inline_max`）。
  - 流式上传（`blob.put` 从路径接入；P13/P07 用到时再加读取形态——本单先收 `&[u8]`，接口留 `put_reader` 的扩展点由那时决定）。
  - `gqy doctor --blobs` 的 CLI（P11-05）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-store/src/blob.rs`（新增） | `BlobStore`：put/get/path_for/BlobCorrupt |
| `crates/gqy-store/src/durable.rs`（新增） | `sync_dir`、`create_dir_all_synced` |
| `crates/gqy-store/src/lib.rs` | 模块声明与文档 |
| `crates/gqy-store/tests/blob.rs`（新增） | 见“测试与守护” |

## 接口草案

草案，以实现为准。

```rust
// gqy-store/src/blob.rs

/// 一个数据目录的 blob 仓库（10 §6.2）。线程安全：`put` 可在 blocking 池并发调用。
pub struct BlobStore { blobs_dir: PathBuf, tmp_dir: PathBuf }

impl BlobStore {
    pub fn new(blobs_dir: PathBuf) -> Self;

    /// 写入：流式写 `tmp/<ulid>` → `sync_data` → 已有则比大小、
    /// 一致删临时文件（去重）、不一致报 `BlobCorrupt` 留两份取证 →
    /// 不存在则 `rename` 后同步父目录。返回内容哈希。
    ///
    /// # Errors
    /// 目录不可写、改名失败且非“目标已存在”、内容长度不一致等。
    pub fn put(&self, bytes: &[u8]) -> Result<BlobHash, BlobError>;

    /// 读取并校验哈希（每次读都校验，10 §6.2）。
    ///
    /// # Errors
    /// 不存在 → `NotFound { hash }`；对不上 → `Corrupt { hash, path }`（文件不动）。
    pub fn get(&self, hash: &BlobHash) -> Result<Vec<u8>, BlobError>;

    /// 目标路径（GC、备份与 doctor 用）。
    pub fn path_for(&self, hash: &BlobHash) -> PathBuf;
    pub fn tmp_dir(&self) -> &Path;
}
```

```rust
// gqy-store/src/durable.rs

/// 同步一个目录（rename/新建之后，父目录也要同步——gqy 3-3 实测的教训：
/// 只同步文件不同步目录，断电后这一项可能丢）。
pub fn sync_dir(dir: &Path) -> io::Result<()>;

/// 逐层建目录；新建的每一层都同步它的上一层。
pub fn create_dir_all_synced(dir: &Path) -> io::Result<()>;
```

## 实施步骤

1. `durable.rs`：两个助手 + 测试（新建目录、已存在目录、父目录同步的可观察行为：只测“调用与不报错”，断电不可测——与 gqy 3-3 一样在单里写明）。
2. `blob.rs`：`put` 最短路径（写 tmp → sync → rename → sync 父目录）；再加去重与冲突分支。
3. `get` + 校验；`path_for`。
4. 测试矩阵（见下）；先写关键负例再实现，确认先红后绿。
5. 与 `Store` 的联动用例：`write` 一笔业务行引用 blob（用 `0001` 里的 `blobs` 登记表做一次插入）；若 0001 尚未合入，先跳过并在 PR 注明补测位置。
6. `cargo xtask check --fast`、`cargo test -p gqy-store`；提交：`feat(store): 内容寻址 blob 存储`。

## 测试与守护

1. **往返**：随机字节（含 `\0`、`\r\n`、非 UTF-8）写入读回一字不差；路径为 `blobs/<h[0..2]>/<h[2..4]>/<64 hex>`，无冒号（Windows 文件名兼容）。
2. **哈希**：与 `blake3(bytes)` 的 64 位小写 hex 一致；`tmp/` 写完后为空。
3. **去重**：同一内容写两遍 → 只有一份文件；第二遍把 mtime 刷成现在（断言“回到现在附近”，不比精确值——平台精度不同）。
4. **崩溃残留**：`tmp/` 里预置一个写了一半的临时文件 → `get(该内容的哈希)` 报 `NotFound` → 再 `put` 同一内容 → `get` 得到完整内容；残留临时文件不阻塞新写入（撞名换名）。
5. **改名竞态**：目标已存在且大小一致 → 视为成功，删掉自己的临时文件（两个写者并发同内容，用多线程跑一遍）。
6. **损坏**：手动改坏目标文件 → `get` 报 `Corrupt { hash, path }`，**文件不被改动、不被删除**（10 §6.2“不静默替换”）。
7. **目标已存在但大小不同** → `BlobCorrupt` 保留两份取证（与 5 区分：大小对照）。
8. **区分能力**：去掉读取校验 → 用例 6 红；`rename` 前不 `sync_data`、去重时直接覆盖 → 相关用例红（或在“不可测”说明里注明为读码检查项）。
9. **边界**：空内容（0 字节）可写可读；大文件（例如 8 MiB+）经 blocking 池写入不卡 async（耗时不断言）。

## 验收流程

```sh
cargo test -p gqy-store          # 全绿；测试名对应上表
cargo xtask check --fast         # 全绿
# 手检（可选）：临时目录里 put 一段内容后
#   find <temp>/blobs -type f      # 只有一个 64 hex 文件；tmp/ 为空
# 先红后绿对照（PR 贴输出）：
#   1) 读取不校验 → 损坏用例红 → 恢复
#   2) 去重时不比大小直接覆盖 → 大小冲突用例红 → 恢复
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 写入五步顺序与 10 §6.2 一致（tmp → sync_data → 判定已有 → rename → 同步父目录）
- [ ] 读取必校验；损坏时“报错、不动文件”
- [ ] 去重、竞态、崩溃残留、大小冲突四类用例齐备
- [ ] `durable.rs` 是两个复用助手（P01-08 与日志路径复用），不是散落的 `fsync`
- [ ] 与 `Store` 的“先落 blob、后提交引用行”联动用例（或注明补测位置）

## 风险与回退

- **mtime 刷新**：去重时刷 mtime 是 10 §6.2 的已定行为（给 GC 宽限期用）；测试只断言“回到现在附近”，不比精确值。
- **断电不可测**：`sync_data`/`sync_dir` 的效果无法在测试里证明（无断电注入）；与 gqy 3-2/3-3 一致，靠代码读审 + “调用存在”的可测部分（例如不报错、目录项行为）兜底，并在单里明说。
- **Windows 文件句柄**：临时文件关闭后再 `rename`；测试里句柄用完就关（Windows 上开着不能改名/删）。
- **回退**：新增模块，revert 即可；blob 目录是新建的，不动已有数据。

