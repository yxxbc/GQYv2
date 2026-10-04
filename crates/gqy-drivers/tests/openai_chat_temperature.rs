//! OpenAI 兼容对话接口的采样温度（施工 8-22，`docs/blueprint/models.md`「驱动要守的约定」第 14 条）：没写的不加；
//! 紧接在输出上限之后、思考强度之前写入；整数值写成整数（0 写入 0，1 写入 1，2 写入 2），小数值写最短精确表示。

mod support;

use std::collections::BTreeMap;

use gqy_drivers::Inputs;
use gqy_drivers::openai_chat::{Compat, encode};
use gqy_kernel::event::Real;
use gqy_kernel::request::{Message, Request};
use support::{call, deepseek, text, texts};

fn request() -> Request {
    Request {
        tools: Vec::new(),
        system: "You are a helpful assistant.".to_string(),
        messages: vec![Message::User {
            blocks: vec![text("你好")],
        }],
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

fn body(temperature: Option<f64>, effort: Option<&str>, compat: &Compat) -> String {
    let mut call = call(Inputs::default(), Some(4096));
    call.temperature = temperature.map(Real::new);
    call.effort = effort.map(str::to_string);
    let encoded = encode(&request(), &call, compat, &texts(), &BTreeMap::new())
        .expect("不要 blob 的请求编码得了");
    String::from_utf8(encoded.body).expect("请求是 UTF-8")
}

#[test]
fn without_a_temperature_nothing_is_added() {
    for compat in [Compat::default(), deepseek()] {
        let plain = body(None, None, &compat);
        assert!(!plain.contains("temperature"), "{plain}");
    }
}

#[test]
fn temperature_is_formatted_and_placed_before_effort() {
    for compat in [Compat::default(), deepseek()] {
        let t0 = body(Some(0.0), None, &compat);
        assert!(t0.ends_with(r#","temperature":0}"#), "{t0}");

        let t07 = body(Some(0.7), None, &compat);
        assert!(t07.ends_with(r#","temperature":0.7}"#), "{t07}");

        let t1 = body(Some(1.0), None, &compat);
        assert!(t1.ends_with(r#","temperature":1}"#), "{t1}");

        let t2 = body(Some(2.0), None, &compat);
        assert!(t2.ends_with(r#","temperature":2}"#), "{t2}");

        let with_effort = body(Some(0.7), Some("high"), &compat);
        assert!(
            with_effort.ends_with(r#","temperature":0.7,"reasoning_effort":"high"}"#),
            "{with_effort}"
        );
    }
}
