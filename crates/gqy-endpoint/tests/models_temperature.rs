//! 模型默认采样温度在端点和协议上的表现（`docs/blueprint/models.md`「协议」「怎么走」第十四条，施工 8-22）：
//! `model.list` 的 `facts` 多 `temperature`，带着完整的配置键名 `key`（带点模型名双引号转义）；
//! 配置里数值超出 `[0, 2]` 范围的报 `out_of_range`、算进 `config_errors`。

mod support;

use std::sync::Arc;

use serde_json::json;

use gqy_endpoint::Core;
use gqy_endpoint::config::{Config, Environment};
use gqy_models::matching::Vendors;
use gqy_models::profile::Profiles;
use gqy_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UseSettings,
};
use gqy_session::testkit::Script;
use gqy_session::{ModelData, Models, Observed, Routes};
use gqy_store::resources::ResourceRoot;
use gqy_tool::Catalog as ToolCatalog;

use support::*;

/// 配置文件：`a/m` 配了 0.7；`a/x` 没配 temperature（配了 tools）；`a/bad` 配了 2.5（超出范围）。
const CONFIG: &str = "[ui]\nlanguage = \"en\"\n\n\
[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"https://a.invalid\"\n\n\
[providers.a.models.m]\ntemperature = 0.7\n\n\
[providers.a.models.x]\ntools = true\n\n\
[providers.a.models.bad]\ntemperature = 2.5\n\n\
[models]\nchat = \"a/m\"\n";

/// 没有目录、读完了的模型资料。
fn no_catalog() -> Arc<ModelData> {
    let data = ModelData::new(
        Profiles::parse(&json!({})).expect("档案写法对"),
        Vendors::default(),
        None,
    );
    data.loaded(None, Observed::default());
    Arc::new(data)
}

/// 一份核心：清单带上模型这一块，配置照磁盘上现在的几份读。
fn core(home: &Home, routed: bool) -> Arc<Core> {
    let data = no_catalog();
    let models: Arc<dyn Models> = match routed {
        true => Arc::new(Routes {
            client: gqy_http::client(gqy_http::Proxy::Off).expect("造得出客户端"),
            direct: gqy_http::client(gqy_http::Proxy::Off).expect("造得出客户端"),
            data: Arc::clone(&data),
            idle: std::time::Duration::from_secs(5),
        }),
        false => Arc::new(Script::new([])),
    };
    let items = [
        gqy_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        PoolSettings::ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        CatalogSettings::ITEMS,
    ]
    .concat();
    let config = Config::load(&home.root, &alice(), None, items, Environment::of(&[]));
    let core = Core::new(
        home.root.clone(),
        ResourceRoot::at(default_resources()),
        models,
        ToolCatalog::default(),
        None,
        alice(),
        TOKEN.to_string(),
    );
    Arc::new(core.with_config(config).with_model_data(data))
}

#[tokio::test]
async fn model_list_shows_the_default_temperature_and_its_full_config_key() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let reply = client.call("l", "model.list", json!({})).await;
    let models = reply["result"]["providers"][0]["models"]
        .as_array()
        .expect("有模型");
    let facts = |name: &str| {
        models
            .iter()
            .find(|model| model["model"] == name)
            .map(|model| model["facts"].clone())
            .unwrap_or_else(|| panic!("没有 {name}"))
    };
    let line = CONFIG
        .lines()
        .position(|line| line == "temperature = 0.7")
        .expect("写了")
        + 1;
    assert_eq!(
        facts("m")["temperature"],
        json!({
            "value": 0.7, "from": "config", "file": "system/config.toml", "line": line,
            "layer": "system", "key": "providers.a.models.m.temperature",
        })
    );
    assert_eq!(
        facts("x")["temperature"],
        json!({"value": null, "from": "default", "key": "providers.a.models.x.temperature"}),
        "没配默认温度的返回 null，但 key 正常给出"
    );
}

/// 模型名带点的：key 引号照 TOML 的写法补上。
#[tokio::test]
async fn the_key_quotes_a_model_name_with_dots_for_temperature() {
    let home = Home::new();
    let config = "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://dev.invalid\"\n\n\
        [providers.dev.models.\"deepseek-v4.1-flash\"]\ntemperature = 0.5\n";
    home.write("system/config.toml", config);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let reply = client.call("l", "model.list", json!({})).await;
    let models = reply["result"]["providers"][0]["models"]
        .as_array()
        .expect("有模型");
    let entry = models
        .iter()
        .find(|model| model["model"] == "deepseek-v4.1-flash")
        .expect("有这个模型");
    assert_eq!(
        entry["facts"]["temperature"]["key"],
        json!("providers.dev.models.\"deepseek-v4.1-flash\".temperature")
    );
    assert_eq!(entry["facts"]["temperature"]["value"], json!(0.5));
}

/// 配置的温度不在 [0, 2] 范围里的报 out_of_range。
#[tokio::test]
async fn temperature_out_of_range_is_reported_as_config_error() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    let hello = client.hello().await;
    assert_eq!(hello["result"]["config_errors"], 1, "{hello}");
    let reply = client.call("g", "config.get", json!({})).await;
    let problems = reply["result"]["problems"].as_array().expect("有问题");
    assert_eq!(problems.len(), 1, "{reply}");
    let problem = &problems[0];
    assert_eq!(
        (&problem["code"], &problem["level"], &problem["key"]),
        (
            &json!("out_of_range"),
            &json!("error"),
            &json!("providers.a.models.bad.temperature")
        ),
        "{problem}"
    );
}
