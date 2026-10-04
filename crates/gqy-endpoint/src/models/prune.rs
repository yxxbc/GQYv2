//! 下架的模型移出池（`docs/blueprint/models.md`「怎么走」第十五条、`config.md`「怎么走」第五条第 13 条，施工 8-23）：
//! `model.list` 一开头清一遍——把记着的下架了的那几个（供应商、模型），从各层各池的 `pools.<名字>.models` 里删掉，写回
//! 写着它的那一层。
//!
//! - 各层各看各的：一层里有几处改动一次写完（[`set::core_set`]：重读、只改那几项、先写临时文件再替换、留痕、推送，
//!   `via` 是 `core`）；删掉一处记一行 `INFO pool member removed`。
//! - 成员照写的原样读（`Place::PoolMember`），供应商、模型都对上记着的才删；读不成、指向别家的不动（逐字节匹配）。
//! - 写成了的、没写在任何一层里的从记着的里去掉（[`ModelData::forget_delisted`]）；有一层写不成的记一行
//!   `WARN pool not removed`，留着，下一条 `model.list` 再试。

use std::collections::BTreeSet;

use gqy_config::Layer;
use gqy_config::Value;
use gqy_models::reference::{Place, Reference};
use gqy_session::ModelData;

use crate::Core;
use crate::config::{Config, set};

/// 运行日志的目标（`models.md`「出错」）。
const TARGET: &str = "gqy::endpoint";

/// 池的成员那一项在清单里的键（`gqy-models` 的 `PoolSettings` 声明的样子，带着占位）。
const POOL_MODELS: &str = "pools.<id>.models";

/// 清一遍：把记着的下架模型从所有池里删掉、写回（施工 8-23）。没有记着的什么都不做。
pub(crate) fn prune(core: &Core, data: &ModelData) {
    let pending: BTreeSet<(String, String)> = data.delisted().into_iter().collect();
    if pending.is_empty() {
        return;
    }
    // 有一层写不成的：这一批留着，下一条 `model.list` 再试。
    let mut failed: BTreeSet<(String, String)> = BTreeSet::new();
    for layer in [Layer::System, Layer::Personal] {
        let removal = {
            let config = core.config();
            to_remove(&config, layer, &pending)
        };
        if removal.changes.is_empty() {
            continue;
        }
        match set::core_set(core, layer, &removal.changes) {
            Ok(()) => {
                for (pool, member) in &removal.removed {
                    tracing::info!(target: TARGET, pool = %pool, member = %member, "pool member removed");
                }
            }
            Err(why) => {
                tracing::warn!(target: TARGET, error = %why, "pool not removed");
                failed.extend(pending.iter().cloned());
            }
        }
    }
    let forget: Vec<(String, String)> = pending
        .iter()
        .filter(|pair| !failed.contains(*pair))
        .cloned()
        .collect();
    data.forget_delisted(&forget);
}

/// 这一层要改的：哪个真的键改成什么，和一共删掉了哪些（池、成员原话）。
struct Removal {
    /// 要改的 `(真的键, 新的成员列表)`：一层里的几处一次写完。
    changes: Vec<(String, Value)>,
    /// 删掉的 `(池, 成员原话)`：记运行日志用。
    removed: Vec<(String, String)>,
}

/// 照这一刻这一层文件里写着的字，算出要改成什么：`pools.<名字>.models` 里读得出、又在 `pending` 里的成员删掉。
fn to_remove(config: &Config, layer: Layer, pending: &BTreeSet<(String, String)>) -> Removal {
    let file = config.file(layer);
    let mut changes = Vec::new();
    let mut removed = Vec::new();
    for (key, entry) in &file.parsed.entries {
        if entry.item != POOL_MODELS || !entry.counts {
            continue;
        }
        let Value::List(members) = &entry.value else {
            continue;
        };
        let Some(name) = pool_name(key) else {
            continue;
        };
        let mut kept: Vec<Value> = Vec::new();
        for member in members {
            let text = match member {
                Value::Text(text) => text.to_string(),
                _ => {
                    kept.push(member.clone());
                    continue;
                }
            };
            match Reference::parse_at(&text, Place::PoolMember) {
                Ok(Reference::Model { provider, model }) => {
                    if pending.contains(&(provider, model)) {
                        removed.push((name.clone(), text));
                    } else {
                        kept.push(member.clone());
                    }
                }
                _ => kept.push(member.clone()),
            }
        }
        if kept.len() != members.len() {
            changes.push((key.clone(), Value::List(kept)));
        }
    }
    Removal { changes, removed }
}

/// 真的键 `pools.<名字>.models` 里那个名字。
fn pool_name(key: &str) -> Option<String> {
    let segments = gqy_config::key::split(key)?;
    let [first, name, last] = segments.as_slice() else {
        return None;
    };
    (first == "pools" && last == "models").then(|| name.clone())
}
