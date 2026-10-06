// @ts-check
//! 登录那一屏（施工 网页并进，核心 W-8）：表单上写哪几句、用户名密码先在页面这头挡一道（核心那边也挡，这里挡是少跑一个来回）。
//! 这一份不碰 DOM，测得到。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { form, complains } from '../../../../resources/web/pages/src/kernel/login.js';
import { t } from '../../../../resources/web/pages/src/util/res.js';

loadRes();

test('登录那一屏：问人登录的写「登录」，用一次性码设密码的写「设一个…」并带一句说明', () => {
  const login = form(t, false);
  assert.equal(login.title, '登录');
  assert.equal(login.hint, '', '登录那一屏不带说明');
  assert.ok(login.submit && login.user && login.password, '三句都要有字');

  const setup = form(t, true);
  assert.match(setup.title, /设.*用户名/);
  assert.ok(setup.hint, '设密码那一屏要有一句说明');
  assert.notEqual(setup.submit, login.submit, '按钮的字不一样');
});

test('用户名、密码先在页面这头挡一道：用户名小写字母开头、最多 32 个字符；密码 8 到 1024、不能只有空白', () => {
  assert.equal(complains('admin', 'eight-chars', t), '', '合规矩的放过去');
  assert.equal(complains('a_b-9', 'gqy-web-test-password', t), '', '下划线、减号、数字都行');
  assert.equal(complains('a'.repeat(32), 'long-enough', t), '', '32 个字符正好');
  for (const user of ['Admin', '9abc', 'a b', 'a.b', '-a', '']) {
    assert.match(complains(user, 'long-enough', t), /小写字母/, `${user} 该被挡`);
  }
  assert.match(complains('a'.repeat(33), 'long-enough', t), /32/, '33 个字符该被挡（这一条是核心的规矩，原来漏了）');
  assert.match(complains('admin', '', t), /8 个字符/);
  assert.match(complains('admin', '1234567', t), /8 个字符/);
  assert.match(complains('admin', ' '.repeat(10), t), /空白/);
  assert.match(complains('admin', 'x'.repeat(1025), t), /1024/);
  assert.equal(complains('admin', 'x'.repeat(1024), t), '', '1024 个字符正好');
});

test('三种语言的登录字都齐（哪一种语言下都不该写 undefined）', async () => {
  const { settle, res } = await import('../../../../resources/web/pages/src/util/res.js');
  for (const code of ['zh', 'ja', 'en']) {
    const json = await import(`node:fs`).then((fs) => JSON.parse(fs.readFileSync(
      new URL(`../../../../resources/web/pages/resources/text/${code}.json`, import.meta.url), 'utf8')));
    const said = json.login;
    for (const key of ['title', 'user', 'password', 'submit', 'bad_user', 'long_user', 'short_password', 'long_password', 'blank_password']) {
      assert.ok(typeof said?.[key] === 'string' && said[key], `${code} 少了 login.${key}`);
    }
    for (const key of ['title', 'hint', 'submit']) {
      assert.ok(typeof said?.setup?.[key] === 'string' && said.setup[key], `${code} 少了 login.setup.${key}`);
    }
    // 装一遍这种语言，表单也写得出来
    settle(json, null, { code, fallback: code, summary: code }, [], json);
    assert.ok(form(res.t ?? t, true).title, `${code} 的表单写得出来`);
  }
});
