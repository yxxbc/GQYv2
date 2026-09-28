//! 日志清理：删除超过保留期的日志文件（19 §3.2 的 14 天保留；P00-05）。
//!
//! 规则与边界见 [`purge_old_logs`]；只动本模块命名的文件
//! （`daemon.*.jsonl*` 与 `requests.*.jsonl*`），别的文件一律不碰。
//! 创建：AI 助手（Cline 会话），2026-09-28 23:03:25。

use std::path::Path;

/// 日志清理的结果。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PurgeReport {
    /// 删除的文件数。
    pub deleted: usize,
    /// 删除失败的文件（路径, 原因）；失败不中断其它删除。
    pub failed: Vec<(String, String)>,
}

/// 删除超过保留期的日志文件（按文件名里的本地日期；19 §3.2 的 14 天保留）。
///
/// 规则：文件日期与 `now` 的本地日期相差**超过** `retention_days` 天就删；正好差
/// `retention_days` 天保留（边界有测试）。只动本模块命名的文件
/// （`daemon.*.jsonl*` 与 `requests.*.jsonl*`），别的文件一律不碰。
///
/// # Errors
///
/// 目录读取失败时返回错误；单个文件删除失败进 [`PurgeReport::failed`]，不中断。
pub fn purge_old_logs(
    dir: &Path,
    retention_days: u32,
    now: jiff::Timestamp,
) -> Result<PurgeReport, std::io::Error> {
    let today = super::local_date(now);
    let mut report = PurgeReport::default();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(date) = super::parse_log_file_date(name) else {
            continue;
        };
        let age_days = today.since(date).map_or(0, |span| span.get_days());
        let retention = i32::try_from(retention_days).unwrap_or(i32::MAX);
        if age_days > retention {
            match std::fs::remove_file(&path) {
                Ok(()) => report.deleted += 1,
                Err(err) => report.failed.push((path.display().to_string(), err.to_string())),
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 固定的「现在」：日期字符串都由它推导，测试与本地时区无关。
    fn fixed_now() -> jiff::Timestamp {
        jiff::Timestamp::from_second(1_790_000_000).expect("固定时间戳应当合法")
    }

    /// 相对「现在」偏移若干天的本地日期字符串（在 `Date` 上加减：jiff 的 `Timestamp` 不支持日历单位）。
    fn date_str(shift_days: i64) -> String {
        let today = super::super::local_date(fixed_now());
        today
            .checked_add(jiff::Span::new().days(shift_days))
            .expect("日期加减应当合法")
            .to_string()
    }

    /// 在临时目录里造一个日志文件（内容随便）。
    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), "{}\n").expect("写文件");
    }

    #[test]
    fn purges_files_older_than_retention() {
        let tmp = tempfile::tempdir().expect("临时目录");
        touch(tmp.path(), &format!("daemon.{}.jsonl", date_str(-16)));
        touch(tmp.path(), &format!("requests.{}.jsonl", date_str(-15)));
        touch(tmp.path(), &format!("daemon.{}.jsonl", date_str(0)));

        let report = purge_old_logs(tmp.path(), 14, fixed_now()).expect("清理应当成功");
        assert_eq!(report.deleted, 2, "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(
            tmp.path()
                .join(format!("daemon.{}.jsonl", date_str(0)))
                .exists()
        );
        assert!(
            !tmp.path()
                .join(format!("daemon.{}.jsonl", date_str(-16)))
                .exists()
        );
    }

    #[test]
    fn keeps_boundary_day_and_foreign_files() {
        let tmp = tempfile::tempdir().expect("临时目录");
        touch(tmp.path(), &format!("daemon.{}.jsonl", date_str(-14))); // 边界：保留
        touch(tmp.path(), "notes.txt"); // 不是日志文件：不动
        touch(tmp.path(), "daemon.not-a-date.jsonl"); // 名字不合规：不动

        let report = purge_old_logs(tmp.path(), 14, fixed_now()).expect("清理应当成功");
        assert_eq!(report.deleted, 0, "{report:?}");
        assert!(
            tmp.path()
                .join(format!("daemon.{}.jsonl", date_str(-14)))
                .exists()
        );
        assert!(tmp.path().join("notes.txt").exists());
        assert!(tmp.path().join("daemon.not-a-date.jsonl").exists());
    }

    #[test]
    fn purges_rotated_suffixes() {
        let tmp = tempfile::tempdir().expect("临时目录");
        touch(tmp.path(), &format!("daemon.{}.3.jsonl", date_str(-20)));

        let report = purge_old_logs(tmp.path(), 14, fixed_now()).expect("清理应当成功");
        assert_eq!(report.deleted, 1, "{report:?}");
    }

    #[test]
    fn missing_directory_is_an_error() {
        let tmp = tempfile::tempdir().expect("临时目录");
        let missing = tmp.path().join("not-there");
        assert!(purge_old_logs(&missing, 14, fixed_now()).is_err());
    }
}
