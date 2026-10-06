// @ts-check
//! 登录（核心施工 W-8，`web-module.md`「怎么走」第一条，施工 网页并进）：**凭据由核心验**，页面自己把凭据带在握手里。
//! 三种凭据照先后（`protocol.md`「握手」第 3 条，正好写一种）：
//!
//! 1. 地址里的 `#setup=<一次性码>`（`gqy web` 打出来的网址带的）：第一次设用户名和密码，或者忘了密码重设。
//! 2. 这台设备上存着的**登录令牌**（`localStorage`，30 天）：平时进站用它。
//! 3. 都没有、或者令牌用不了了：问人拿**用户名和密码**，核心验过以后回一个登录令牌存起来。
//!
//! 这一份只管「怎么问人、怎么把话说给核心」；存在哪、什么时候用哪一种在 `src/host/browser.js` 的宿主里。
//! 页面的字走 `text/<语言>.json` 的 `login`（三份语言都写）。

/**
 * @typedef {{user: string, password: string}} Password 问人拿到的用户名和密码
 * @typedef {(key: string, fields?: any) => string} T 界面上的字（`util/res.js` 的 `t`）
 * @typedef {{title: string, hint: string, submit: string, user: string, password: string}} Form 一屏表单要写的那几句
 */

/**
 * 一屏表单上要写的那几句（`t` 是 `util/res.js` 的 `t`：编号 → 字）。抽出来是为了不被 DOM 挡住就能测。
 * @param {T} t
 * @param {boolean} setup 真的用一次性码设密码？
 * @returns {Form}
 */
export function form(t, setup) {
  const title = setup ? t('login.setup.title') : t('login.title');
  return {
    title,
    hint: setup ? t('login.setup.hint') : '',
    submit: setup ? t('login.setup.submit') : t('login.submit'),
    user: t('login.user'),
    password: t('login.password'),
  };
}

/**
 * 问人拿用户名和密码。交回之前页面就停在这一屏上（`root` 上只有这一个表单）。
 *
 * 用户名要合账号的写法（核心 W-8：小写字母开头，只有小写字母、数字、`-`、`_`）；密码核心要求 8 位以上，这里先照它挡一道，
 * 免得多跑一个来回。
 * @param {HTMLElement} root 页面的根
 * @param {T} t
 * @returns {Promise<Password>}
 */
export function askPassword(root, t) {
  return ask(root, t, false);
}

/**
 * 用一次性码设用户名和密码（第一次、忘了密码重设）。核心收下以后交回一个登录令牌，调用方存起来。
 * @param {HTMLElement} root 页面的根
 * @param {T} t
 * @returns {Promise<Password>}
 */
export function askSetup(root, t) {
  return ask(root, t, true);
}

/**
 * 那一屏表单。
 * @param {HTMLElement} root
 * @param {T} t
 * @param {boolean} setup
 */
function ask(root, t, setup) {
  const what = form(t, setup);
  return new Promise((resolve) => {
    root.classList.add('login');
    const form = document.createElement('form');
    form.className = 'login';
    form.innerHTML = `
      <h1>${escape(what.title)}</h1>
      ${what.hint ? `<p class="login-hint">${escape(what.hint)}</p>` : ''}
      <label>${escape(what.user)}<input name="user" autocomplete="username" autofocus></label>
      <label>${escape(what.password)}<input name="password" type="password" autocomplete="current-password"></label>
      <p class="login-bad" hidden></p>
      <button type="submit">${escape(what.submit)}</button>`;
    const bad = /** @type {HTMLElement} */ (form.querySelector('.login-bad'));
    const button = /** @type {HTMLButtonElement} */ (form.querySelector('button'));
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      if (what.done) return;
      const data = new FormData(form);
      const user = String(data.get('user') ?? '').trim();
      const password = String(data.get('password') ?? '');
      const wrong = complains(user, password, t);
      if (wrong) {
        bad.textContent = wrong;
        bad.hidden = false;
        return;
      }
      what.done = true;
      button.disabled = true;
      resolve({ user, password });
    });
    root.replaceChildren(form);
  });
}
/**
 * 用户名、密码先说一遍（核心的规矩，`kernel/id.rs` 的 `check_name`、`login.rs` 的 `good_password`：账号名小写字母开头、
 * 只有小写字母数字 `-` `_`、**最多 32 个字符**（还得避开 Windows 的保留名，那个列表太长，交给核心）；密码 8 到 1024 个字符、
 * 不能只有空白）。不行的话说哪不对，不让这个来回白跑。
 *
 * 这里和核心那两处是一条规矩的两个地方：核心改了（同一个主版本内不该改），这里跟着改，`login.test.js` 守着。
 * @param {string} user
 * @param {string} password
 * @param {T} t
 */
export function complains(user, password, t) {
  if (!/^[a-z][a-z0-9_-]*$/.test(user)) return t('login.bad_user');
  if (user.length > 32) return t('login.long_user');
  if (password.length < 8) return t('login.short_password');
  if (password.length > 1024) return t('login.long_password');
  if (!password.trim()) return t('login.blank_password');
  return '';
}

/** 放进 HTML 里的字先转义（用户名、密码是外面来的，别的都是资源里的字）。 */
function escape(text) {
  return String(text).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c] ?? c);
}
