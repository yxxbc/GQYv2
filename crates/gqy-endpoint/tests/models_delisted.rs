//! 下架的模型移出池（施工 8-23，`docs/blueprint/models.md`「怎么走」第十五条、`config.md`「怎么走」第五、六条、「协议」）：
//! 供应商的列表里原来有、这一份成功拉到的里没有的模型，`model.list` 一开头从各层各池的成员里删掉、写回（推
//! `config.changed`，`via` 是 `core`、不带 `by`；留痕 `by` 是内核、没有 `cause`）；删空的池留着、带 `problem`；
//! 从没列过的不动；拉不到的不动。写不成的那一条在 `models_delisted_log.rs`（要接运行日志）。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use gqy_endpoint::Core;
use gqy_endpoint::config::{Config, Environment};
use gqy_http::testkit::{Piece, Reply, Server};
use gqy_http::{Proxy, fetcher};
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

/// 系统配置、个人设置在数据根里的位置。
const SYSTEM: &str = "system/config.toml";
const PERSONAL: &str = "home/alice/settings.toml";

/// 供应商列表的回应：`ids` 里那几个模型。
fn page(ids: &[&str]) -> Reply {
    let models: Vec<String> = ids.iter().map(|id| format!(r#"{{"id":"{id}"}}"#)).collect();
    Reply::stream(vec![Piece::Bytes(
        format!(r#"{{"data":[{}]}}"#, models.join(",")).into_bytes(),
    )])
}

/// 拉不到的回应。
fn failed() -> Reply {
    Reply::error(500, &[], r#"{"error":{"message":"boom"}}"#)
}

/// 一家 `relay`：假服务器、没有 key；两个池，成员里有 `relay/x`、`relay/y`，和从没列过的 `relay/manual`。
fn config(base_url: &str) -> String {
    format!(
        "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkeys = []\n\n\
         # 注释要留着\n[pools.free]\nmodels = [\"relay/x\", \"relay/y\", \"relay/manual\"]\n\n\
         [pools.flat]\nmodels = [\"relay/x\"]\n"
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
    let reply = client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    client
}

/// 问一次 `model.list` 带 `refresh`：交回读到的推送和回应。
async fn refresh(client: &mut Client, id: &str) -> (Vec<Value>, Value) {
    let params = json!({"refresh": true});
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "model.list", "params": params});
    client.line(&request.to_string()).await;
    let (pushed, reply) = client.until_reply(id).await;
    assert!(reply["result"].is_object(), "{reply}");
    (pushed, reply)
}

/// `model.list` 回应里的池 `name`。
fn pool<'a>(list: &'a Value, name: &str) -> &'a Value {
    list["pools"]
        .as_array()
        .and_then(|pools| pools.iter().find(|pool| pool["name"] == name))
        .unwrap_or_else(|| panic!("没有池 {name}：{list}"))
}

/// 这一份文件现在的字。
fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).expect("读得到")
}

/// 一份日志里的每一条。
fn journal(home: &Home, relative: &str) -> Vec<Value> {
    std::fs::read_to_string(home.root.path().join(relative))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行一条 JSON"))
        .collect()
}

/// 下架的从所有池里删掉、写回配置文件（别的字节不动）、推送、留痕；删空的池留着；从没列过的不动。
#[tokio::test]
async fn a_delisted_model_leaves_every_pool_and_the_file_says_so() {
    let server = Server::start(vec![page(&["x", "y"]), page(&["y"])]).await;
    let home = Home::new();
    home.write(SYSTEM, &config(&server.base_url));
    let data = data();
    let core = core(&home, Arc::clone(&data));
    let mut client = subscribed(Arc::clone(&core)).await;
    // 第一份：x、y 都在列表里。没有「原来有」，池子和文件都不动。
    let (pushed, first) = refresh(&mut client, "l1").await;
    assert!(pushed.is_empty(), "{pushed:?}");
    assert_eq!(
        pool(&first["result"], "free")["models"],
        json!(["relay/x", "relay/y", "relay/manual"])
    );
    assert!(pool(&first["result"], "free").get("problem").is_none());
    assert!(
        read(&home, SYSTEM).contains("relay/manual"),
        "第一份没有下架的"
    );
    // 第二份：x 没了。从两个池里删掉，写回文件。
    let (pushed, second) = refresh(&mut client, "l2").await;
    assert_eq!(
        pool(&second["result"], "free")["models"],
        json!(["relay/y", "relay/manual"]),
        "这一次就照清完的答；从没列过的 manual 不动"
    );
    assert_eq!(pool(&second["result"], "flat")["models"], json!([]));
    assert_eq!(
        pool(&second["result"], "flat")["problem"],
        json!("pool \"flat\" has no models"),
        "删空的池留着，标成用不了"
    );
    let text = read(&home, SYSTEM);
    assert!(
        text.contains("models = [\"relay/y\", \"relay/manual\"]"),
        "{text}"
    );
    assert!(text.contains("models = []"), "{text}");
    assert!(text.contains("# 注释要留着"), "别的字节不动：{text}");
    // 推送：一层一条，via 是 core、不带 by。
    assert_eq!(pushed.len(), 1, "{pushed:?}");
    let push = &pushed[0]["params"];
    assert_eq!(push["layer"], "system");
    assert_eq!(push["via"], "core");
    assert!(push.get("by").is_none(), "核心自己改的不带 by：{push}");
    assert_eq!(
        push["keys"]["pools.free.models"]["value"],
        json!(["relay/y", "relay/manual"])
    );
    assert_eq!(push["keys"]["pools.flat.models"]["value"], json!([]));
    // 留痕：by 是内核、没有 cause。
    let entries = journal(&home, "system/journal.jsonl");
    let last = entries.last().expect("记了一条");
    assert_eq!(last["kind"], "config.changed");
    assert_eq!(last["by"], json!({"kind": "kernel"}));
    assert!(last.get("cause").is_none(), "{last}");
    assert_eq!(last["body"]["via"], "core");
    assert_eq!(last["body"]["changes"][0]["key"], "pools.flat.models");
    assert_eq!(last["body"]["changes"][1]["key"], "pools.free.models");
}

/// 系统配置、个人设置各看各的：两层的池都清，各推一条。
#[tokio::test]
async fn each_layer_is_cleaned_on_its_own() {
    let server = Server::start(vec![page(&["x", "y"]), page(&["y"])]).await;
    let home = Home::new();
    home.write(SYSTEM, &config(&server.base_url));
    home.write(PERSONAL, "[pools.mine]\nmodels = [\"relay/x\"]\n");
    let data = data();
    let core = core(&home, Arc::clone(&data));
    let mut client = subscribed(Arc::clone(&core)).await;
    refresh(&mut client, "l1").await;
    let (pushed, _) = refresh(&mut client, "l2").await;
    assert_eq!(pushed.len(), 2, "一层一条：{pushed:?}");
    assert_eq!(pushed[0]["params"]["layer"], "system");
    assert_eq!(pushed[1]["params"]["layer"], "personal");
    assert_eq!(
        read(&home, PERSONAL),
        "[pools.mine]\nmodels = []\n",
        "个人设置里的也清"
    );
    assert!(read(&home, SYSTEM).contains("relay/y"));
    assert_eq!(
        journal(&home, "home/alice/journal.jsonl")
            .last()
            .expect("记了一条")["body"]["layer"],
        "personal"
    );
}

/// 拉不到的：这一份不算数，池子和文件都不动。
#[tokio::test]
async fn a_failed_fetch_changes_nothing() {
    let server = Server::start(vec![page(&["x", "y"]), failed()]).await;
    let home = Home::new();
    home.write(SYSTEM, &config(&server.base_url));
    let data = data();
    let core = core(&home, Arc::clone(&data));
    let mut client = subscribed(Arc::clone(&core)).await;
    refresh(&mut client, "l1").await;
    let before = read(&home, SYSTEM);
    let (pushed, second) = refresh(&mut client, "l2").await;
    assert!(pushed.is_empty(), "{pushed:?}");
    assert_eq!(
        pool(&second["result"], "free")["models"],
        json!(["relay/x", "relay/y", "relay/manual"])
    );
    assert_eq!(read(&home, SYSTEM), before, "文件一个字节不动");
}
