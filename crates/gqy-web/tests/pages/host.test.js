// @ts-check
//! 宿主（蓝图 `web/architecture.md`「宿主」「多用户、多终端」，施工 网页并进）：平台的 API 只在 `src/host/` 里用；
//! 浏览器那一份的凭据由核心验（W-8）；本机文件、blob 走网页软件的 `/media`（W-10）；这台设备上存的东西按账号分开；
//! 连核心只认一条「像 WebSocket」的线，桌面端换一条线就能用。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { urls, browserHost } from '../../../../resources/web/pages/src/host/browser.js';
import { accountStorage } from '../../../../resources/web/pages/src/kernel/storage.js';
import { Connection, Refusal } from '../../../../resources/web/pages/src/core/connection.js';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../resources/web/pages');

/** 目录下全部 `.js`。 */
function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? files(p) : p.endsWith('.js') ? [p] : [];
  });
}

/** 平台的 API、网页软件的地址（「宿主」的「页面自己守的」第 1 条） */
const PLATFORM = [
  /new WebSocket\b/, /\blocation\.hash\b/, /\blocalStorage\b/, /\bsessionStorage\b/, /\bwindow\.open\(/, /\bnavigator\.clipboard\b/,
  /\bdataTransfer\b/, /['"`]\/(ws|upload|media)\b/,
];

test('平台的 API、网页软件的地址只在 src/host/ 里用', () => {
  const host = join(ROOT, 'src', 'host');
  const bad = [];
  for (const file of [...files(join(ROOT, 'src')), ...files(join(ROOT, 'packages'))]) {
    if (file.startsWith(host)) continue;
    readFileSync(file, 'utf8').split('\n').forEach((line, i) => {
      // 代码行上的才管：注释（`//`、`//!`、`/**`、`*` 开头的）都剥掉（注释里提到地址不算）
      const code = line.replace(/^\s*(\/\/!?|\/\*\*?|\*).*$/, '').replace(/\/\/.*$/, '');
      for (const re of PLATFORM) if (re.test(code)) bad.push(`${relative(ROOT, file)}:${i + 1} ${re}`);
    });
  }
  assert.deepEqual(bad, []);
});

test('浏览器那一份的地址走 `/media`（W-10）：先交空串，票据回来了才给；下载带 download，blob 另带原来的名字', () => {
  const u = urls();
  // 票据要 POST 换，还没换回来时先交空串（页面照它不画，换回来了页面上重来一次）
  assert.equal(u.file('/home/a b/x.png'), '');
  assert.equal('linkImage' in u, false, '链接卡片的图是 blob，照 `/media` 取（核心施工 W-7）');
});

test('凭据照先后（W-8）：存着的登录令牌、地址里的一次性码、都没有时交 null（页面问人）', () => {
  assert.equal(browserHost().credentials(), null, '什么都没存：交 null，页面问人要用户名密码');
});

test('凭据一次只给一种：换成登录令牌以后，那个用过的一次性码不再跟着握（核心回 bad_params 的那一次）', () => {
  const host = browserHost();
  // 照页面的路：先拿一次性码，再用它换登录令牌
  host.usePassword('admin', 'gqy-web-test-password');
  assert.deepEqual(host.credentials(), { user: 'admin', password: 'gqy-web-test-password' }, '先问人拿到的');
  host.useLogin('login-token', null);
  const after = host.credentials();
  assert.deepEqual(after, { login: 'login-token' }, '换到登录令牌以后只带它一种');
  const keys = Object.keys(after ?? {});
  for (const other of ['code', 'user', 'password']) {
    assert.equal(keys.includes(other), false, `不该同时带 ${other}`);
  }
});

/** 一个假的存法：一张表，`broken` 时读写都抛（隐私窗口、清过数据） */
function fakeStore(broken = false) {
  const table = new Map();
  return {
    table,
    getItem: (k) => { if (broken) throw new Error('no'); return table.get(k) ?? null; },
    setItem: (k, v) => { if (broken) throw new Error('no'); table.set(k, v); },
    removeItem: (k) => { table.delete(k); },
  };
}

test('这台设备上存的按账号分开：键带账号，换一个账号看不到（「多用户、多终端」第 5 条）', () => {
  const store = fakeStore();
  const admin = accountStorage(store, 'admin');
  admin.set('packages', { rail: { disabled: true } });
  assert.equal(store.table.get('gqy.admin.packages'), '{"rail":{"disabled":true}}');
  assert.deepEqual(admin.get('packages', {}), { rail: { disabled: true } });
  assert.deepEqual(accountStorage(store, 'alice').get('packages', {}), {});
});

test('存法读写抛错、存的不是 JSON 的，照没有算，不让页面起不来', () => {
  assert.equal(accountStorage(fakeStore(true), 'admin').get('x', 7), 7);
  accountStorage(fakeStore(true), 'admin').set('x', 1);
  const store = fakeStore();
  store.table.set('gqy.admin.x', '{坏的');
  assert.equal(accountStorage(store, 'admin').get('x', 7), 7);
});

test('原来不分账号的旧键，第一次读时搬到这个账号名下', () => {
  const store = fakeStore();
  store.table.set('gqy.packages', '{"todo":{"disabled":true}}');
  const admin = accountStorage(store, 'admin', { packages: 'gqy.packages' });
  assert.deepEqual(admin.get('packages', {}), { todo: { disabled: true } });
  assert.equal(store.table.has('gqy.packages'), false);
  assert.equal(store.table.get('gqy.admin.packages'), '{"todo":{"disabled":true}}');
});

/** 一条假的线：样子照 WebSocket（`send`、`readyState`、`onopen`、`onmessage`、`onclose`、`onerror`） */
function fakeChannel() {
  const ch = { readyState: 0, sent: /** @type {any[]} */ ([]), onopen: null, onmessage: null, onclose: null, onerror: null,
    send(text) { this.sent.push(JSON.parse(text)); } };
  return ch;
}

test('连核心只认一条像 WebSocket 的线：通了才发，回应对上请求，拒绝是 Refusal，断了等着的都拒掉', async () => {
  const ch = /** @type {any} */ (fakeChannel());
  const conn = new Connection(() => ch);
  const states = [];
  conn.onStatus((s) => states.push(s));
  const up = conn.connect();
  await assert.rejects(conn.request('x'), (e) => e instanceof Refusal && e.reason === 'disconnected');
  ch.readyState = 1;
  ch.onopen();
  await up;
  const got = conn.request('session.list', {});
  const [m] = ch.sent;
  assert.equal(m.method, 'session.list');
  ch.onmessage({ data: JSON.stringify({ jsonrpc: '2.0', id: m.id, result: { ok: 1 } }) });
  assert.deepEqual(await got, { ok: 1 });
  const refused = conn.request('session.send', {});
  ch.onmessage({ data: JSON.stringify({ jsonrpc: '2.0', id: ch.sent[1].id, error: { code: -32010, message: '不行', data: { reason: 'turn_running' } } }) });
  await assert.rejects(refused, (e) => e instanceof Refusal && e.reason === 'turn_running');
  const pending = conn.request('slow', {});
  ch.readyState = 3;
  ch.onclose();
  await assert.rejects(pending, (e) => e.reason === 'disconnected');
  assert.deepEqual(states, ['connecting', 'online', 'offline']);
});

test('断了自己重连：隔一会儿再开一条线，连上了告诉外面（重新握手、补上漏掉的由外面做）；一开始就连不上的不在这里重连', async () => {
  const lines = /** @type {any[]} */ ([]);
  const conn = new Connection(() => { const ch = fakeChannel(); lines.push(ch); return ch; }, [0, 0]);
  const states = [];
  conn.onStatus((s) => states.push(s));
  let reopened = 0;
  conn.onReopen(() => { reopened += 1; });
  const up = conn.connect();
  lines[0].readyState = 1;
  lines[0].onopen();
  await up;
  // 核心重启：桥把线关了
  lines[0].readyState = 3;
  lines[0].onclose();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(lines.length, 2, '又开了一条');
  // 这一次也没连上（核心还没起来）：接着试
  lines[1].onerror();
  lines[1].onclose();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(lines.length, 3, '再试一次');
  lines[2].readyState = 1;
  lines[2].onopen();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(reopened, 1, '连上了告诉外面一次');
  assert.deepEqual(states, ['connecting', 'online', 'offline', 'connecting', 'offline', 'connecting', 'online']);
});

test('重连：连不上就一直按退避试（并进以后没有口令这回事）；宿主说没用了就不再试，告诉外面一声', async () => {
  const lines = /** @type {any[]} */ ([]);
  let more = true;
  const conn = new Connection(() => { const ch = fakeChannel(); lines.push(ch); return ch; }, [0, 0], async () => more);
  let lost = 0;
  conn.onLost(() => { lost += 1; });
  const up = conn.connect();
  lines[0].readyState = 1;
  lines[0].onopen();
  await up;
  lines[0].readyState = 3;
  lines[0].onclose();
  await new Promise((r) => setTimeout(r, 5));
  lines[1].onerror();
  lines[1].onclose();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(lines.length, 3, '还能连：接着试');
  more = false;
  lines[2].onerror();
  lines[2].onclose();
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(lost, 1, '不能再连：说一声');
  assert.equal(lines.length, 3, '不再试');
});
