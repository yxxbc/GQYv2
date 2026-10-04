//! 模型默认温度在路由上的透传（`docs/blueprint/models.md`「怎么走」第十四条，施工 8-22）：
//! 一次请求照真发的那个模型配置的默认温度带，没有就不带；换模型以后用新模型自己的；
//! 轮换的池里每个成员用自己的；个人设置压着系统配置，改了下一轮生效。
//!
//! 两台假服务器，驱动为 openai-chat。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch as channel;

use gqy_config::Layer;
use gqy_config::merge::{Layers, Resolved, merge};
use gqy_config::parse::parse;
use gqy_http::testkit::Server;
use gqy_kernel::session::Command;
use gqy_session::{ConfigSource, Handle, Models, fixed_with};
use gqy_tool::Catalog;
use support::routing::{configs, hellos, items, routes};
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

/// 两家 `a`、`b`，都不带 key。`a` 的 `m` 默认温度 0.7；`b` 的 `n` 没有配温度。池 `p`
/// 是两个都有的轮换。`models.chat` 是 `a/m`。
fn config(first: &Server, second: &Server, a_temp: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\n{a_temp}\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.n]\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n\n[models]\nchat = \"a/m\"\n",
        first.base_url, second.base_url
    )
}

/// 造一个会话，记着的模型是 `model`。
async fn create(home: &Home, models: &dyn Models, model: &str) -> Handle {
    let lines = Lines {
        model: Some(model.to_string()),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 说一句、等这一轮说完。
async fn turn(handle: &Handle, command: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    let _ = until_turn_ends(&mut pushes).await;
}

/// 换模型。
async fn switch(handle: &Handle, command: &str, model: &str) {
    let configure = Command::Configure {
        model: model.to_string(),
    };
    ask(handle, command, configure).await.expect("会话在跑");
}

/// 一台服务器收到的每一份请求里的 `temperature`；没带的是 `None`。
fn sent(server: &Server) -> Vec<Option<f64>> {
    server
        .received()
        .iter()
        .map(|received| {
            let body: serde_json::Value =
                serde_json::from_slice(&received.body).expect("请求是 JSON");
            body.get("temperature").and_then(serde_json::Value::as_f64)
        })
        .collect()
}

/// 请求带上配置的默认温度；换模型后用新模型自己的。
#[tokio::test]
async fn a_request_takes_the_configured_temperature_default_or_nothing() {
    let (first, second) = (
        Server::start(hellos(12)).await,
        Server::start(hellos(12)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, "temperature = 0.7\n"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;

    turn(&handle, "cmd-1").await;
    assert_eq!(
        sent(&first).first().copied().flatten(),
        Some(0.7),
        "带上配置的默认温度 0.7"
    );

    // 换模型以后用新模型自己的：`b/n` 没有默认温度，不带。
    switch(&handle, "cmd-2", "b/n").await;
    turn(&handle, "cmd-3").await;
    assert_eq!(
        sent(&second).last().copied().flatten(),
        None,
        "没有配温度的模型不带"
    );
}

/// 轮换池里的每个成员用各自配置的温度。
#[tokio::test]
async fn each_member_of_a_rotating_pool_uses_its_own_temperature() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let text = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\ntemperature = 0.2\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.n]\ntemperature = 1.5\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n\n[models]\nchat = \"a/m\"\n",
        first.base_url, second.base_url
    );
    let mut home = Home::new();
    home.configs = configs(&text, &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "@p").await;

    turn(&handle, "cmd-1").await;
    turn(&handle, "cmd-2").await;

    let (a, b) = (sent(&first), sent(&second));
    assert!(
        !a.is_empty() && !b.is_empty(),
        "两个成员都发过：{a:?} {b:?}"
    );
    assert!(a.iter().all(|temp| *temp == Some(0.2)), "a/m 用 0.2：{a:?}");
    assert!(b.iter().all(|temp| *temp == Some(1.5)), "b/n 用 1.5：{b:?}");
}

/// 系统配置 `system`、个人设置 `personal` 两层合出来的最终值，当成一份不变的配置源。
fn two_layers(system: &str, personal: &str) -> Arc<dyn ConfigSource> {
    let item_list = items();
    let system = parse(&item_list, Layer::System, system).expect("写法对");
    let personal = parse(&item_list, Layer::Personal, personal).expect("写法对");
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        ..Layers::default()
    };
    let resolved: Resolved = merge(&item_list, &layers, &|_| None);
    Arc::clone(&*fixed_with(resolved, Vec::new()).borrow())
}

/// 个人设置压着系统配置：造好时系统配置的 0.7 生效；改了个人设置下一轮生效。
#[tokio::test]
async fn a_personal_setting_overrides_the_system_temperature_on_the_next_turn() {
    let (first, second) = (
        Server::start(hellos(12)).await,
        Server::start(hellos(12)).await,
    );
    let system = config(&first, &second, "temperature = 0.7\n");
    let (switching, receiving) = channel::channel(two_layers(&system, ""));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;

    turn(&handle, "cmd-1").await;
    assert_eq!(
        sent(&first).first().copied().flatten(),
        Some(0.7),
        "先是系统配置的 0.7"
    );

    // 个人设置改为 0.0：这一轮后下一轮生效。
    let personal = "[providers.a.models.m]\ntemperature = 0.0\n";
    switching.send_replace(two_layers(&system, personal));

    turn(&handle, "cmd-2").await;
    assert_eq!(
        sent(&first).last().copied().flatten(),
        Some(0.0),
        "个人设置压着系统配置，下一轮生效为 0.0"
    );
}
