// @ts-check
//! 连不上核心时写什么（蓝图 `web.md`「连核心」第 9 条，施工 网页并进）：为什么、怎么办。
//! 并进以后不再有桥：连不上只有两种——核心（经 `gqy-web`）没在跑，或者连上了、核心那头自己报了错。纯函数。

import { t } from '../util/res.js';

/**
 * @param {'none'|'down'|'core'} kind
 * @param {string} [message] 核心的原话（`core` 时）
 * @returns {{title: string, why: string, how: string}}
 */
export function offline(kind, message = '') {
  const title = t('boot.title');
  if (kind === 'core') return { title, why: message, how: t('boot.core_how') };
  return { title, why: t('boot.down_why'), how: t('boot.down_how') };
}
