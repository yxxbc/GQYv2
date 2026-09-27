<div align="center">
  <img src="assets/pics/gqy-logo.png" alt="GQY logo" width="160" />
  <h1>GQY v2 · 顾清影</h1>
  <p><strong>Rust as the body, Web as the face, C as the path</strong></p>
  <p><a href="README.md">简体中文</a> ｜ English</p>
</div>

**GQY (顾清影, "Selene")** is an AI assistant that lives on your own computer. She can chat with you. She can also read code, edit files, run commands and handle everyday tasks. Your sessions, memories and permissions stay on your machine, and you decide where the boundaries are.

GQY v2 is a ground-up rebuild of [gqy-agent (v1)](https://github.com/yxxbc/gqy-agent). v1 proved the idea works, but its foundations became too hard to maintain. v2 rebuilds them in pure Rust. The goal is a single program you download and run.

> [!IMPORTANT]
> **Status: design phase. Not installable yet.**
> The repository currently holds design documents, a construction plan and engineering rules. There is no runnable program yet. If you want to use GQY today, keep using [v1](https://github.com/yxxbc/gqy-agent). The first v2 release will ship a v1 data importer (see the [roadmap](#roadmap)).

## What she is meant to be

Everything below is a **design goal** for v2. None of it is an existing feature.

- **One file, ready to run.** The core is a single executable. You don't need Python, Node.js or any other runtime installed.
- **Terminal and browser.**
  - A terminal UI (TUI).
  - A web console built into the binary, which also works on phones and tablets.
  - An optional desktop app.
  - All three work on the same sessions, so you can start a conversation in one and continue it in another.
- **Stable over long runs.** Sessions can last for days, with contexts in the 100k-token range. Memory use, latency and cost should stay predictable.
- **Affordable long conversations.** Requests are strictly append-only, so they hit the provider's prefix cache as often as possible. v1 measured a 99.9% hit rate on long sessions.
- **You stay in control.**
  - Capability modes: read-only, workspace, and full.
  - GQY asks before any action with consequences.
  - Commands run inside a sandbox. If the sandbox cannot be enforced, GQY refuses to run the command.
- **Nothing gets lost.**
  - Your data stays on your machine.
  - Upgrades only apply additive migrations.
  - Importing v1 data goes through a dry run and a verification step before anything is written.
- **Devices help each other.** Every app is both a client and a server, with no central server required.
- **Companion features return later.** Personas, memory, and chat platforms such as QQ and iMessage come back once the core is stable.

## Platforms

| Platform | Plan |
| --- | --- |
| Linux | Full support |
| macOS (Apple silicon) | Full support |
| Windows | Compiles with reduced features. Anything that needs a missing capability (such as the sandbox) is refused. |

## Roadmap

| Milestone | What you can do | Status |
| --- | --- | --- |
| Design & construction plan | Read the design and join the discussion | In progress |
| M1 Minimal loop | Have a conversation with a model in the terminal, stored locally | Not started |
| M2 Daily coding | Read code, edit files and run commands in the terminal, with approvals and a sandbox | Not started |
| M3 Web console | Use GQY in a browser, on the same sessions as the terminal | Not started |
| M4 Companion | Personas, memory, QQ / iMessage | Not started |
| M5 Multi-device | Pair devices, share capabilities, sync on request | Not started |
| M6 First release | Download and install, import v1 data | Not started |

## Documentation

The documentation is written in Chinese.

| Document | Contents |
| --- | --- |
| [Technical whitepaper](docs/GQYv2-Technical-Whitepaper.md) | Background and overall approach |
| [Design documents](docs/designs/00-设计理念.md) | Read in numbered order, starting from `00-设计理念.md` |
| [Construction plan](docs/construction/00-施工总纲.md) | Phases, milestones and per-step work orders |
| [Tech stack](docs/tech-stack.md) | Languages and technology choices |
| [Release & versioning](docs/release-versioning.md) | Version numbers and release process |

## Contributing

Design discussion is welcome, and so is help implementing the work orders:

- Contribution workflow: [CONTRIBUTING.md](CONTRIBUTING.md).
- Rules for AI assistants: [AGENTS.md](AGENTS.md).
- Report security issues privately as described in [SECURITY.md](SECURITY.md). Please don't open a public issue.

## License

The source code is licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE). You may use, modify and share it for any noncommercial purpose; commercial use is not permitted. The character 顾清影 / Selene and the brand assets (logo, wallpaper, mascot images) are covered separately by [LICENSE-ASSETS](LICENSE-ASSETS). Contribution terms are in [CONTRIBUTING.md](CONTRIBUTING.md).
