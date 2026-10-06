// @ts-check
//! 连不上核心时写的两句（蓝图 `web.md`「连核心」第 9 条，施工 网页并进）。并进以后不再有桥：核心经网页软件连，
//! 连不上只有两种——核心（经 `gqy-web`）没在跑，或者连上了、核心那头自己报了错（照它的原话）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { offline } from '../../../../resources/web/pages/src/model/offline.js';

loadRes();

test('核心没在跑：说怎么起', () => {
  const down = offline('down');
  assert.equal(down.title, '连不上核心');
  assert.match(down.why, /没在跑|连不上核心/);
  assert.match(down.how, /gqy web/);
  assert.equal(offline('none').how, down.how, '没凭据也是同一套话：起 gqy web');
});

test('核心那头报了错：照原话，叫看终端里的报错', () => {
  const core = offline('core', '找不到数据根：没有权限');
  assert.equal(core.why, '找不到数据根：没有权限');
  assert.match(core.how, /终端里的报错/);
});
