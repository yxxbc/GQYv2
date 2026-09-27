# 应用入口

`apps/` 仅放用户交互端和应用宿主。当前目录是规划占位，不代表这些应用已经实现。

```text
apps/
  tui/              Rust + Ratatui/Crossterm 终端界面
  web-console/src/  TypeScript Web 控制台源码，规划为编译时嵌入 Rust 二进制
  desktop/src-tauri/ 可选 Tauri v2 桌面宿主
```

Axum 网关、Agent Harness、存储和系统 FFI 属于共享运行时，不在此目录拆成独立服务；未来若抽成 Rust crates，应放在仓库根目录的 `crates/`。这些应用入口暂不创建独立 Cargo/npm 包，也不改变根目录 Release Please 的单一 `0.1.0` 版本。
