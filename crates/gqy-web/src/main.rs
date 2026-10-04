//! 程序 `gqy-web`（`web-module.md`「怎么走」第九条、第十一条，施工 W-9）：`open` 是 `gqy web` 交来的，`serve` 是 `open`
//! 拉起来的，不写进帮助。
//!
//! - `gqy-web serve [--port <端口>]`
//! - `gqy-web open [--port <端口>] [--print] [--reset] [--logout]`

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;

use gqy_store::env::Env;
use gqy_store::resources::ResourceRoot;
use gqy_store::root::DataRoot;
use gqy_web::open::{Launch, Open, SystemBrowser, open};
use gqy_web::serve::Serve;
use gqy_web::settings::Settings;

/// 用法不对的退出码。
const USAGE: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        return usage();
    };
    let Some(parsed) = parse(rest) else {
        return usage();
    };
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        eprintln!("gqy-web: no runtime");
        return ExitCode::from(1);
    };
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("gqy-web: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = root.prepare() {
        eprintln!("gqy-web: {error}");
        return ExitCode::from(1);
    }
    match command.as_str() {
        "serve" if !parsed.print && !parsed.reset && !parsed.logout => {
            ExitCode::from(runtime.block_on(serve(root, &env, parsed.port)))
        }
        "open" => {
            let launch = Launch {
                serve: Box::new(|port| {
                    let mut serve = Command::new(myself());
                    serve.arg("serve");
                    if let Some(port) = port {
                        serve.args(["--port", &port.to_string()]);
                    }
                    serve
                }),
                core: Arc::new(core),
            };
            let code = runtime.block_on(open(
                &root,
                &parsed,
                &launch,
                &SystemBrowser,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            ));
            ExitCode::from(code)
        }
        _ => usage(),
    }
}

/// `serve`：读资源、装运行日志，跑到空闲退出。
async fn serve(root: DataRoot, env: &Env, port: Option<u16>) -> u8 {
    let failed = |reason: String| {
        print!("{}", gqy_ipc::Ready::Failed(reason).line());
        1
    };
    let resources = match ResourceRoot::locate(env) {
        Ok(resources) => resources,
        Err(error) => return failed(error.to_string()),
    };
    let settings = match Settings::load(resources.path()) {
        Ok(settings) => settings,
        Err(error) => return failed(error),
    };
    let pages = std::env::var_os("GQY_WEB_PAGES")
        .filter(|pages| !pages.is_empty())
        .map_or_else(|| resources.path().join("web").join("pages"), PathBuf::from);
    let _log = gqy_log::install(
        &root.state().join("logs"),
        "web",
        gqy_log::LevelFilter::INFO,
        env.home.as_deref(),
    );
    let serve = Serve {
        root,
        port: port.unwrap_or(settings.port),
        pages,
        settings,
        core: Arc::new(core),
    };
    let ran = gqy_web::serve::run(serve, |ready| {
        print!("{}", ready.line());
        use std::io::Write;
        if std::io::stdout().flush().is_err() {
            // 拉起它的那一头走了：照常跑。
        }
    })
    .await;
    u8::from(ran.is_err())
}

/// 读参数：`--port <端口>`、`--print`、`--reset`、`--logout`，别的不认。
fn parse(args: &[String]) -> Option<Open> {
    let mut open = Open::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => open.port = Some(args.next()?.parse().ok()?),
            "--print" => open.print = true,
            "--reset" => open.reset = true,
            "--logout" => open.logout = true,
            _ => return None,
        }
    }
    Some(open)
}

/// 用法不对。
fn usage() -> ExitCode {
    eprintln!("usage: gqy-web open [--port <port>] [--print] [--reset] [--logout]");
    ExitCode::from(USAGE)
}

/// 自己在哪。
fn myself() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gqy-web"))
}

/// 拉起核心：自己真实位置旁边的主程序 `gqy` 加 `core`（施工 W-9「施工时定的」第 3 条）。
fn core() -> Command {
    let mut core = Command::new(sibling(&myself(), "gqy"));
    core.arg("core");
    core
}

/// `program` 真实位置旁边叫 `name` 的程序（Windows 上加 `.exe`）。
fn sibling(program: &Path, name: &str) -> PathBuf {
    let real = std::fs::canonicalize(program).unwrap_or_else(|_| program.to_path_buf());
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    real.parent()
        .map_or_else(|| PathBuf::from(&file), |dir| dir.join(&file))
}
