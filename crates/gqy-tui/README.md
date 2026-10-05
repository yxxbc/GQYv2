## 终端界面

全屏的终端界面，连上核心跟她说话。现在长什么样，见蓝图 `docs/blueprint/tui.md`。

在仓库工作区里（`crates/gqy-tui`）：门禁照查，lints 照工作区那一套。

### 怎么跑

先在工作树根目录编一次核心（`cargo build -p gqy`），再：

```sh
cd crates/gqy-tui
DEEPSEEK_API_KEY=<key> GQY_HOME=$HOME/.cache/gqy-tui-home GQY_RESOURCES=$PWD/../../resources GQY_CORE_BIN=$PWD/../../target/debug/gqy cargo run
```

- 变量写在命令前面，不要 `export`：旧版 GQY 也认 `GQY_HOME`。
- `GQY_HOME` 放在持久目录里，别放 `/tmp`：`/tmp` 是 tmpfs，一重启模型配置、会话记录全没（2026-10-02 撞见过一次）。
- 拉起核心只认 `GQY_CORE_BIN`，不去 PATH 里找 `gqy`（PATH 上的可能是旧版）。核心已经在跑的，不给也能连上。
- 上下文窗口照核心给的：订阅的回应里带着会话的限额 `limits`（`window`、`compaction_line`），头不自己查。旧版演示自己那份窗口表 `models.json` 已经删了。

### 按键、鼠标、版式

都写在蓝图 `docs/blueprint/tui.md` 里（「对外的样子」「怎么走」「样子」），这里不再抄一份。

### herdr 中恢复会话

`gqy-tui --resume <完整会话 ID>` 恢复指定会话，优先于 `tui.startup=recent`。在 herdr 中创建或切换主会话后自动上报恢复命令；服务重启后每个窗格恢复各自的会话。`/new` 清掉旧绑定。只退出 herdr 客户端时原进程继续运行。

恢复程序 `gqy-tui` 必须在恢复 shell 的 PATH 中，GQY_HOME、GQY_RESOURCES、GQY_CORE_BIN 也须可用；临时写在原启动命令前的环境变量不会被 herdr 保存。试用可用固定这些环境的启动脚本，放到 PATH 中；独立命名 herdr 会话先试，不停止默认服务。

Linux 隔离验收（需要 herdr）：`GQY_HERDR_LIVE=1 cargo test -p gqy-tui --test herdr_resume -- --ignored`。两道锁（`#[ignore]` 加这个环境变量，和 `gqy-net/tests/live.rs` 的 `GQY_NET_LIVE` 同一个写法）：CI 的长跑用 `--ignored` 跑所有标了的测试，没装 herdr 的机器上会跳过。使用临时数据根、假模型和独立 XDG 配置，验证两窗格各自恢复、/new 清掉旧绑定，不读写真实会话。
