//! Responses 接口的采样温度（施工 8-22，`docs/blueprint/models.md`「驱动要守的约定」第 14 条）：
//! 思考档位未启用（没有思考或 off）时，紧接在 `max_output_tokens` 之后、思考强度之前写入；
//! 开启思考档位时，驱动抑制温度字段，不写入，避免 API 报错。

mod support;

use std::collections::BTreeMap;

use gqy_drivers::openai_responses::encode;
use gqy_drivers::{EFFORT_OFF, Inputs};
use gqy_kernel::block::Block;
use gqy_kernel::event::Real;
use gqy_kernel::request::{Message, Request};
use support::{gpt, text, texts};

fn request(reply: Vec<Block>) -> Request {
    Request {
        tools: vec![],
        system: "You are GQY.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("想一想")],
            },
            Message::Assistant { blocks: reply },
            Message::User {
                blocks: vec![text("接着")],
            },
        ],
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

fn body(temperature: Option<f64>, effort: Option<&str>) -> String {
    let mut call = gpt(Inputs::default(), Some(4096));
    call.temperature = temperature.map(Real::new);
    call.effort = effort.map(str::to_string);
    let encoded = encode(
        &request(vec![text("好的。")]),
        &call,
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob 的请求编码得了");
    String::from_utf8(encoded.body).expect("UTF-8")
}

#[test]
fn without_a_temperature_nothing_is_added() {
    let plain = body(None, None);
    assert!(!plain.contains("temperature"), "{plain}");
}

#[test]
fn temperature_sent_when_reasoning_effort_is_inactive() {
    let t07 = body(Some(0.7), None);
    assert!(
        t07.ends_with(r#","max_output_tokens":4096,"temperature":0.7}"#),
        "{t07}"
    );

    let t_off = body(Some(0.7), Some(EFFORT_OFF));
    assert!(
        t_off.ends_with(
            r#","max_output_tokens":4096,"temperature":0.7,"reasoning":{"effort":"none"}}"#
        ),
        "{t_off}"
    );
}

#[test]
fn temperature_suppressed_when_reasoning_effort_is_active() {
    let t_high = body(Some(0.7), Some("high"));
    assert!(!t_high.contains("temperature"), "{t_high}");
    assert!(
        t_high.contains(r#""reasoning":{"effort":"high""#),
        "{t_high}"
    );
}
