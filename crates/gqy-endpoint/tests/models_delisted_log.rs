//! 下架的模型移出池的运行日志（施工 8-23，`docs/blueprint/models.md`「出错」）：删掉一处记 `INFO pool member removed`；
//! 写不成（配置所在的目录只读）记 `WARN pool not removed`，池不动，记着的下一条 `model.list`（不拉）再试、清掉。
//!
//! 全局装一个写进内存的订阅者：全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。写不成那一段
//! 要一个只读的目录，只有 Unix 上做得到：这个文件整个只在 Unix 上编（`config_set.rs` 的 `a_write_that_fails_changes_nothing`
//! 也是只读这一手）。
#![cfg(unix)]

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use gqy_endpoint::Core;
use gqy_endpoint::config::{Config, Environment};
use gqy_http::testkit::{Piece, Reply, Server};
use gqy_http::{Proxy, fetcher};
use gqy_log::{LevelFilter, Memory};
use gqy_models::matching::Vendors;
use gqy_models::profile::Profiles;
use gqy_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UseSettings,
};
use gqy_session::testkit::Script;
use gqy_session::{ModelData, Observed};
use gqy_store::resources::ResourceRoot;
use gqy_tool::Catalog as ToolCatalog;

use support::*;

/// 供应商列表的回应：`ids` 里那几个模型。
fn page(ids: &[&str]) -> Reply {
    let models: Vec<String> = ids.iter().map(|id| format!(r#"{{"id":"{id}"}}"#)).collect();
    Reply::stream(vec![Piece::Bytes(
        format!(r#"{{"data":[{}]}}"#, models.join(",")).into_bytes(),
    )])
}

/// 一家 `relay`：假服务器、没有 key；一个池，成员里有 `relay/x`、`relay/y`。
fn config(base_url: &str) -> String {
    format!(
        "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkeys = []\n\n\
         [pools.free]\nmodels = [\"relay/x\", \"relay/y\"]\n"
    )
}

/// 一份模型资料：没有目录，能拉列表。
fn data() -> Arc<ModelData> {
    let data = ModelData::new(Profiles::default(), Vendors::default(), None)
        .with_fetcher(fetcher(Proxy::Off).expect("造得出客户端"));
    data.loaded(None, Observed::default());
    Arc::new(data)
}

/// 一份核心：清单带上模型这一块，配置照磁盘上现在的几份读。
fn core(home: &Home, data: Arc<ModelData>) -> Arc<Core> {
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
        Arc::new(Script::new([])),
        ToolCatalog::default(),
        None,
        alice(),
        TOKEN.to_string(),
    );
    Arc::new(core.with_config(config).with_model_data(data))
}

/// 连上、握手、订阅配置的推送。
async fn subscribed(core: Arc<Core>) -> Client {
    let mut client = Client::connect(core);
    client.hello().await;
    client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    client
}

/// 问一次 `model.list`：`params` 是那几格，交回读到的推送和回应。
async fn ask(client: &mut Client, id: &str, params: Value) -> (Vec<Value>, Value) {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "model.list", "params": params});
    client.line(&request.to_string()).await;
    let (pushed, reply) = client.until_reply(id).await;
    assert!(reply["result"].is_object(), "{reply}");
    (pushed, reply)
}

/// 这一份文件现在的字。
fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).expect("读得到")
}

#[tokio::test]
async fn a_removal_and_a_failed_write_are_logged() {
    use std::os::unix::fs::PermissionsExt;
    let memory = Memory::new();
    tracing::subscriber::set_global_default(gqy_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let has = |what: &str| memory.lines().iter().any(|line| line.contains(what));

    let server = Server::start(vec![page(&["x", "y"]), page(&["y"])]).await;
    let home = Home::new();
    home.write("system/config.toml", &config(&server.base_url));
    // 配置所在的目录只读：写得进临时文件才怪。
    let dir = home.root.path().join("system");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("改得了");
    let data = data();
    let core = core(&home, Arc::clone(&data));
    let mut client = subscribed(Arc::clone(&core)).await;
    ask(&mut client, "l1", json!({"refresh": true})).await;
    let (pushed, second) = ask(&mut client, "l2", json!({"refresh": true})).await;
    assert!(pushed.is_empty(), "写不成：不推、不记日志：{pushed:?}");
    assert!(
        read(&home, "system/config.toml").contains("relay/x"),
        "写不成：池不动"
    );
    assert_eq!(
        second["result"]["pools"][0]["models"],
        json!(["relay/x", "relay/y"]),
        "这一次照手头的答"
    );
    assert!(has("pool not removed"), "{:?}", memory.lines());
    assert!(!has("pool member removed"), "{:?}", memory.lines());
    // 能写了：下一条 `model.list`（不拉）把记着的清掉。
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("改得了");
    let (pushed, third) = ask(&mut client, "l3", json!({})).await;
    assert_eq!(pushed.len(), 1, "{pushed:?}");
    assert_eq!(pushed[0]["params"]["via"], "core");
    assert!(
        !read(&home, "system/config.toml").contains("relay/x"),
        "清掉了"
    );
    assert_eq!(
        third["result"]["pools"][0]["models"],
        json!(["relay/y"]),
        "照清完的答"
    );
    assert!(
        has("pool member removed pool=free member=relay/x"),
        "{:?}",
        memory.lines()
    );
}
