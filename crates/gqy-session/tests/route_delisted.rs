//! 下架的认法（施工 8-23，`docs/blueprint/models.md`「怎么走」第十五条）：换上一份新拉的供应商列表时，上一份里有、
//! 这一份里没有的记下来（供应商、模型）；拉回来的从记着的里去掉；这家从没拉过列表的不算（没有原来有）；清完了的
//! 用 `forget_delisted` 去掉。都在内存里，不落盘。

use std::sync::Arc;

use gqy_kernel::time::Timestamp;
use gqy_models::matching::Vendors;
use gqy_models::observed::{ListedModel, ProviderList};
use gqy_models::profile::Profiles;
use gqy_session::ModelData;

/// 一份列表：`ids` 里那几个模型，时刻固定。
fn list(ids: &[&str]) -> ProviderList {
    ProviderList {
        fetched: Timestamp::parse("2026-10-05T08:00:00.000Z").expect("时刻写法对"),
        models: ids
            .iter()
            .map(|id| ListedModel {
                id: (*id).to_string(),
                window: None,
            })
            .collect(),
    }
}

/// 一份模型资料：没有目录（测试里不写盘）。
fn data() -> Arc<ModelData> {
    Arc::new(ModelData::new(
        Profiles::default(),
        Vendors::default(),
        None,
    ))
}

/// 一对供应商、模型。
fn pair(provider: &str, model: &str) -> (String, String) {
    (provider.to_string(), model.to_string())
}

#[test]
fn a_model_that_falls_out_of_the_list_is_remembered_until_it_is_cleared() {
    let data = data();
    assert!(data.delisted().is_empty(), "还没拉过：没有原来有");
    data.set_list("dev", list(&["m", "n"]));
    assert!(data.delisted().is_empty(), "第一份没有可比的");
    data.set_list("dev", list(&["n", "k"]));
    assert_eq!(
        data.delisted(),
        vec![pair("dev", "m")],
        "m 掉出列表记下，新加的 k 不算"
    );
    data.set_list("dev", list(&["n"]));
    assert_eq!(
        data.delisted(),
        vec![pair("dev", "k"), pair("dev", "m")],
        "k 也没了，一起记着；m 那一个还在"
    );
    data.forget_delisted(&[pair("dev", "m")]);
    assert_eq!(data.delisted(), vec![pair("dev", "k")], "忘掉一个");
    data.forget_delisted(&[pair("dev", "k")]);
    assert!(data.delisted().is_empty(), "清完了忘掉");
}

#[test]
fn a_model_that_comes_back_is_not_delisted_any_more() {
    let data = data();
    data.set_list("dev", list(&["m"]));
    data.set_list("dev", list(&[]));
    assert_eq!(data.delisted(), vec![pair("dev", "m")]);
    data.set_list("dev", list(&["m"]));
    assert!(data.delisted().is_empty(), "又列出来了");
}

#[test]
fn each_provider_is_remembered_on_its_own() {
    let data = data();
    data.set_list("dev", list(&["m"]));
    data.set_list("other", list(&["m"]));
    data.set_list("dev", list(&[]));
    assert_eq!(
        data.delisted(),
        vec![pair("dev", "m")],
        "别家的同名不算；dev 的名单没动过"
    );
}
