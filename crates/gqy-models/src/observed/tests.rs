//! 用出来的、供应商的列表：读写一字不差，只记小的（施工 8-7）。

use gqy_kernel::time::Timestamp;

use super::{Learned, ListedModel, ProviderList, Stamped, delisted};

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("时刻写法对")
}

#[test]
fn a_learned_window_is_kept_only_when_it_is_smaller() {
    let mut learned = Learned::default();
    let first = at("2026-10-01T08:12:30.000Z");
    assert!(learned.learn("dev", "v4.1", 65_536, first));
    assert!(!learned.learn("dev", "v4.1", 65_536, at("2026-10-02T00:00:00.000Z")));
    assert!(!learned.learn("dev", "v4.1", 131_072, at("2026-10-02T00:00:00.000Z")));
    assert_eq!(
        learned.window("dev", "v4.1"),
        Some(Stamped {
            value: 65_536,
            at: first
        })
    );
    let later = at("2026-10-03T00:00:00.000Z");
    assert!(learned.learn("dev", "v4.1", 32_000, later));
    assert_eq!(
        learned.window("dev", "v4.1").map(|stamped| stamped.value),
        Some(32_000)
    );
    assert_eq!(learned.window("dev", "other"), None);
    assert_eq!(
        learned.to_json(),
        r#"{"dev/v4.1":{"window":{"value":32000,"at":"2026-10-03T00:00:00.000Z"}}}"#
    );
    assert_eq!(Learned::parse(&learned.to_json()), Ok(learned));
    assert!(Learned::parse("[]").is_err());
}

#[test]
fn a_provider_list_reads_back_the_same() {
    let list = ProviderList {
        fetched: at("2026-10-01T03:00:00.000Z"),
        models: vec![
            ListedModel {
                id: "deepseek-flash".to_string(),
                window: Some(1_000_000),
            },
            ListedModel {
                id: "x".to_string(),
                window: None,
            },
        ],
    };
    let text = list.to_json();
    assert_eq!(
        text,
        r#"{"fetched":"2026-10-01T03:00:00.000Z","models":[{"id":"deepseek-flash","window":1000000},{"id":"x"}]}"#
    );
    assert_eq!(ProviderList::parse(&text), Ok(list.clone()));
    assert_eq!(list.find("x").map(|listed| listed.window), Some(None));
    assert!(list.find("y").is_none());
    assert!(ProviderList::parse("{}").is_err());
}

/// 上一份列表里有、这一份里没有的算下架（施工 8-23，`models.md`「怎么走」第十五条）：新加的、还在的不算；
/// 空的这一份（这一家一个都不列了）全部算下架；重复的名字只算一个，照字节序排。
#[test]
fn a_model_that_was_listed_and_is_gone_is_delisted() {
    let previous = ProviderList {
        fetched: at("2026-10-01T03:00:00.000Z"),
        models: vec![
            listed("deepseek-flash"),
            listed("x"),
            listed("gone"),
            listed("x"),
        ],
    };
    let fresh = vec![listed("x"), listed("new")];
    assert_eq!(
        delisted(&previous, &fresh),
        vec!["deepseek-flash".to_string(), "gone".to_string()]
    );
    assert!(delisted(&previous, &previous.models).is_empty(), "都没有变");
    assert_eq!(
        delisted(&previous, &[]),
        ["deepseek-flash", "gone", "x"].map(str::to_string).to_vec()
    );
}

/// 列表里的一个模型：只有名字，没报窗口。
fn listed(id: &str) -> ListedModel {
    ListedModel {
        id: id.to_string(),
        window: None,
    }
}
