//! `gqy config` 不带子命令时拉起终端界面（施工 8-24，`cli/config.md`「怎么走」第 9 条）：
//! 不在终端里印帮助、退出码 2；在终端里找旁边的 `gqy-tui`，带 `--page config` 拉起、退出码照它；
//! 找不到的说怎么装、退出码 1。

use super::*;

/// 一个用完就删的目录，里面摆一个主程序（`gqy`）和可选的邻居。
struct Site {
    dir: std::path::PathBuf,
}

impl Site {
    fn new(tag: &str) -> Site {
        let dir = std::env::temp_dir().join(format!("gqy-cli-head-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了");
        Site { dir }
    }

    /// 主程序所在的位置（照它找邻居）。
    fn main(&self) -> std::path::PathBuf {
        let main = self
            .dir
            .join(format!("gqy{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&main, "").expect("写得进");
        main
    }

    /// 在 `gqy` 旁边放一个可执行的 `gqy-tui`：把收到的参数写进 `seen`，退出码照 `code`。
    #[cfg(unix)]
    fn neighbour(&self, code: u8) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let seen = self.dir.join("seen");
        let program = self
            .dir
            .join(format!("gqy-tui{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\nexit {code}\n",
                seen.display()
            ),
        )
        .expect("写得进假邻居");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("可执行");
        seen
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.dir));
    }
}

#[test]
fn not_in_a_terminal_prints_help_and_exits_two() {
    let site = Site::new("notty");
    let main = site.main();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = head_on(false, &main, Language::Chinese, &mut out, &mut err);
    assert_eq!(code, 2, "不在终端里：参数不对的退出码");
    let help = String::from_utf8(out).expect("UTF-8");
    assert!(
        help.contains("gqy config") && help.contains("get"),
        "不在终端里时照旧印帮助：{help}"
    );
}

/// 不在终端里时**不该**去拉起界面（连试都不试）。用假邻居看得见它有没有被跑过。
#[cfg(unix)]
#[test]
fn not_in_a_terminal_never_looks_for_the_head() {
    let site = Site::new("notty-lazy");
    let main = site.main();
    let seen = site.neighbour(0);
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        head_on(false, &main, Language::Chinese, &mut out, &mut err),
        2
    );
    assert!(
        !seen.is_file(),
        "不在终端里时不该拉起界面（邻居不该被跑过）"
    );
}

#[test]
fn a_missing_head_says_how_to_install_and_exits_one() {
    let site = Site::new("missing");
    let main = site.main();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = head_on(true, &main, Language::Chinese, &mut out, &mut err);
    assert_eq!(code, 1);
    let said = String::from_utf8(err).expect("UTF-8");
    // 要的是「没装、怎么装」那句，不是拉起失败的那句：两者都由 `Err` 一类的路径给
    // 退出码 1，不盯住这句就区分不开。
    assert!(said.contains("没装") && said.contains("gqy-tui"), "{said}");
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        head_on(true, &main, Language::English, &mut out, &mut err),
        1
    );
    let said = String::from_utf8(err).expect("UTF-8");
    assert!(
        said.contains("not installed") && said.contains("gqy-tui"),
        "英文也说清装哪个包：{said}"
    );
}

/// 界面在、但**跑不起来**（坏二进制、权限不对）：和「没装」分开说，都不能说成成了。
/// 靠「一个空文件当程序」（类 Unix 上 exec 会拒）——只在 Unix 上有这个把握。
#[cfg(unix)]
#[test]
fn a_head_that_will_not_start_is_not_reported_as_missing() {
    let site = Site::new("unrunnable");
    let main = site.main();
    // 一个像程序、但不是能执行的（空文件，类 Unix 上 exec 会拒）。
    let broken = site
        .dir
        .join(format!("gqy-tui{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&broken, "").expect("写得进");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = head_on(true, &main, Language::Chinese, &mut out, &mut err);
    assert_eq!(code, 1, "跑不起来也不成");
    let said = String::from_utf8(err).expect("UTF-8");
    assert!(
        !said.contains("没装") && said.contains("gqy-tui"),
        "在、但跑不起来：不该说成「没装」：{said}"
    );
}

/// 退出码照界面的：邻居说 7 就是 7（不是恒 0、也不是拿 FAiled 顶）。
#[cfg(unix)]
#[test]
fn the_exit_code_is_whatever_the_head_says_not_a_constant() {
    for want in [0u8, 3, 255] {
        let site = Site::new(&format!("code{want}"));
        let main = site.main();
        let _seen = site.neighbour(want);
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            head_on(true, &main, Language::Chinese, &mut out, &mut err),
            want,
            "界面退出码 {want} 要原样带出来"
        );
    }
}

/// 界面被信号杀死（没有退出码）时算没成，不算成 0。
#[cfg(unix)]
#[test]
fn a_head_killed_by_a_signal_is_not_reported_as_success() {
    use std::os::unix::fs::PermissionsExt;
    let site = Site::new("signal");
    let main = site.main();
    let program = site
        .dir
        .join(format!("gqy-tui{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&program, "#!/bin/sh\nkill -TERM $$\n").expect("写得进");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("可执行");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = head_on(true, &main, Language::Chinese, &mut out, &mut err);
    assert_eq!(code, 1, "没正常退出就算没成，不是 0");
}

/// 在不在终端里：标准和输出**都是**终端才算（一个是一个不是的不算）。
#[test]
fn being_in_a_terminal_needs_both_ends() {
    assert!(super::both(true, true), "两都是终端才算");
    assert!(
        !super::both(true, false),
        "输出被接管道的不算（被脚本调就是这个）"
    );
    assert!(!super::both(false, true), "输入被重定向的不算");
    assert!(!super::both(false, false));
}

#[cfg(unix)]
#[test]
fn an_installed_head_gets_page_config_and_decides_the_exit_code() {
    let site = Site::new("ok");
    let main = site.main();
    let seen = site.neighbour(7);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = head_on(true, &main, Language::Chinese, &mut out, &mut err);
    assert_eq!(code, 7, "退出码照界面的");
    let args = std::fs::read_to_string(&seen).expect("邻居被跑过");
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        ["--page", "config"],
        "带 --page config 拉起"
    );
}

#[test]
fn the_head_is_looked_for_next_to_the_main_program() {
    // `gqy web` 找 `gqy-web` 用的是同一套 `sibling`：这里确认它照真实位置找。
    let dir = std::env::temp_dir().join(format!("gqy-cli-head-sib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    let main = dir.join("gqy");
    std::fs::write(&main, "").expect("写得进");
    // 两边都走 canonicalize（macOS 的 /tmp 是符号链接，Windows 上带 `\\?\` 前缀）：
    // 只比「邻居在主程序旁边」这件事，不比字面串。
    let got = crate::web::sibling(&main, "gqy-tui");
    assert_eq!(
        got.file_name(),
        Some(std::ffi::OsStr::new(&format!(
            "gqy-tui{}",
            std::env::consts::EXE_SUFFIX
        )))
    );
    assert_eq!(
        std::fs::canonicalize(got.parent().expect("有上一层")).expect("在"),
        std::fs::canonicalize(&dir).expect("在")
    );
    drop(std::fs::remove_dir_all(&dir));
}

#[test]
fn a_head_that_fails_to_start_says_so() {
    // 邻居在、但不是能跑的程序（权限不对、坏二进制）：说一句，退出码 1。
    let site = Site::new("broken");
    let main = site.main();
    let broken = site
        .dir
        .join(format!("gqy-tui{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&broken, "").expect("写得进");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = head_on(true, &main, Language::Chinese, &mut out, &mut err);
    assert_eq!(code, 1, "拉不起来也算没成");
    assert!(!err.is_empty(), "要说一句");
}
