// @ts-check
//! 浏览器这个宿主（蓝图 `web/architecture.md`「宿主」，照 Linux 的 `arch/`）：和平台有关的都在这一份里，页面、内核、软件包
//! 不直接碰（测试照源码查）。以后桌面端（Tauri）加一份 `tauri.js`，交出一样的东西，别处不动。
//!
//! 并进主仓库以后（施工 网页并进）经 `gqy-web`（`web-ui.md`），不再有桥：
//! - **身份**（设计 21 X6、核心 W-8）：凭据由**核心**验，页面自己带着走。三种凭据照先后：
//!   地址里的 `#setup=<一次性码>`（第一次设用户名密码，用完从地址栏抹掉）、存着的**登录令牌**、
//!   或者问人拿到的**用户名和密码**。三者都没有时交 `null`，由页面问人（`credentials()`）。
//! - **连核心的线**是一条 WebSocket（`/ws`），不带凭据：握手时把凭据放进 `hello`。
//! - **本机文件、blob**（链接卡片的配图、图标、附件缩略图）走网页软件的 `/media`（W-10）：换一张票据，照票据取。
//! - 链接照网页的写法（`target=_blank`、`download`），浏览器自己会办，`intercept` 什么都不做。

/**
 * @typedef {{name: string, size: number, type: string, file?: Blob, path?: string, stored?: {session: string, hash: string}}} FileRef
 *   `stored`：核心存好的那一份（输入历史翻出来的附件），缩略图、内容照 `/media` 取
 *   一个要当附件的文件：浏览器给的只有内容（`file`），桌面端给的有路径（`path`）
 * @typedef {{readyState: number, send: (text: string) => void, onopen: any, onmessage: any, onclose: any, onerror: any}} Channel
 *   连核心的一条线，样子照 WebSocket：`readyState` 是 1 时通着，一帧一条 JSON-RPC 消息
 * @typedef {{getItem: (k: string) => string|null, setItem: (k: string, v: string) => void, removeItem: (k: string) => void}} Store
 *   这台设备上存东西的存法
 * @typedef {{login?: string, code?: string, user?: string, password?: string}} Credentials
 *   握手时带上哪一种凭据（`protocol.md`「握手」第 3 条：正好写一种）
 */

/** 登录令牌和它的过期时刻存在这里（这台设备、这个浏览器）。 */
const LOGIN = 'gqy.login';
/** 地址里的一次性码：`#setup=<码>`（照 `gqy-web open` 打出来的网址）。 */
const SETUP = /setup=([0-9a-f]+)/;

/** 存着的登录令牌：没有、读不出、已经过期的都算没有（过期的清掉）。不在浏览器里（Node 跑测试）没有。 */
function storedLogin() {
  if (typeof localStorage !== 'object') return null;
  try {
    const raw = localStorage.getItem(LOGIN);
    if (!raw) return null;
    const got = JSON.parse(raw);
    if (typeof got?.token !== 'string' || !got.token) return null;
    if (typeof got.expires === 'string' && Date.parse(got.expires) <= Date.now()) {
      localStorage.removeItem(LOGIN);
      return null;
    }
    return got.token;
  } catch {
    return null;
  }
}

/** 地址里的一次性码：拿到以后从地址栏抹掉（不发给服务器、不进 Referer，设计 21 X6）。不在浏览器里（Node 跑测试）没有。 */
function setupCode() {
  if (typeof location !== 'object') return null;
  const m = location.hash.match(SETUP);
  if (!m) return null;
  history.replaceState(null, '', location.pathname + location.search);
  return m[1];
}

/**
 * 这一份宿主现在拿得出的凭据（`protocol.md`「握手」第 3 条：**正好写一种**）。三种都没有时 `null`：页面问人要用户名和密码。
 * 优先级就是先后：登录令牌 > 一次性码 > 问人拿到的。
 *
 * **每次现读**（不缓存）：登录令牌从 `localStorage` 读、一次性码从地址栏读（读到就抹掉）。
 * 不缓存是有意的：一次性码换成登录令牌以后（`useLogin`），下一次握得只带登录令牌——
 * 带两种凭据核心回 `bad_params`（施工 网页并进 真机撞见）。现读也让测试喂得了假的环境。
 */
let login = /** @type {string|null} */ (null);
let code = /** @type {string|null} */ (null);
let asked = /** @type {{user: string, password: string}|null} */ (null);

/**
 * 网页软件给的 `/media`：把一张票据换成地址用的东西。
 * 造票据要 POST（带登录令牌），所以这一层把「要过的地址」排成一队，等票据回来再补上。
 */
const tickets = new Map();

/** 登录令牌变了（刚设好、刚登录、作废了）时，把之前换过的票据都作废。 */
function forgetTickets() {
  tickets.clear();
}

/** 照票据取一个地址；还没换过、或者换失败了的交回空串。 */
function ticketUrl(key) {
  const got = tickets.get(key);
  return typeof got === 'string' ? got : '';
}

/**
 * 换一张票据（`POST /media`，W-10 第 1、4、5 条）：带登录令牌，正文说清是哪个 blob、哪个路径。
 * 换到了补进 `tickets`，交回 `/media/<票据>`；换不到交回 `null` 并记一句。
 * @param {{blob?: string, path?: string, type?: string, name?: string|null, download?: boolean}} what
 */
async function mint(what) {
  const token = login;
  if (!token) return null;
  try {
    const got = await fetch('/media', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(what),
    });
    if (!got.ok) return null;
    const said = await got.json();
    return typeof said?.url === 'string' ? said.url : null;
  } catch {
    return null;
  }
}

/** 一个 key（说清是哪个资源、怎么取）对应的地址：换过就用，没换过先交空串、在后台换。 */
function media(key, what) {
  if (tickets.has(key)) return ticketUrl(key);
  tickets.set(key, null);
  mint(what).then((url) => {
    tickets.set(key, url);
    // 换回来了：用着这个地址的图画/链接重来一次（页面不重画整屏）
    if (url) document.dispatchEvent(new CustomEvent('gqy-media', { detail: { key, url } }));
  });
  return '';
}

/**
 * `/media` 的几种地址（W-10）：一个本机文件、一个 blob。
 * @param {{file: string, download?: boolean, type?: string, name?: string|null}} what
 */
export function urls() {
  return {
    /** 一个本机文件：`download` 为真的叫浏览器存下来。 */
    file: (/** @type {string} */ path, download = false) =>
      media(`file:${path}:${download}`, { path, download }),
    /** 一个 blob（你的话里的附件）：下载的存成 `name`。 */
    blob: (
      /** @type {string} */ hash,
      /** @type {string} */ type,
      /** @type {{download?: boolean, name?: string|null}} */ how = {},
    ) => media(`blob:${hash}:${type}:${how.download ?? false}:${how.name ?? ''}`, {
      blob: hash,
      type,
      download: how.download,
      name: how.name,
    }),
  };
}

/** 浏览器的文件换成附件用的样子。 */
const refs = (/** @type {Iterable<File>} */ list) => [...list].map((file) => /** @type {FileRef} */ ({ name: file.name, size: file.size, type: file.type, file }));

/** 选文件：系统的选文件窗口，能多选；取消的交回空的。 */
function pick() {
  return new Promise((resolve) => {
    const input = /** @type {HTMLInputElement} */ (document.createElement('input'));
    input.type = 'file';
    input.multiple = true;
    input.addEventListener('change', () => resolve(refs(input.files ?? [])), { once: true });
    input.addEventListener('cancel', () => resolve([]), { once: true });
    input.click();
  });
}

/**
 * 拖文件（蓝图 `web.md`「附件」第 1 条）：文件一拖进窗口 `enter`，到了 `target` 上面、离开它 `over(真假)`，拖出窗口、松开了
 * `leave`；只有在 `target` 上松开才 `drop`（交回文件；附件给的是整页，在哪松开都收）。在别处松开什么都不做，页面也不跳去打开那个文件。拖的不是文件的不管。
 * 交回怎么不看。桌面端照窗口的拖放事件和落点做同一件事。
 * @param {HTMLElement} target 收文件的那一块
 * @param {{enter: () => void, over: (inside: boolean) => void, leave: () => void, drop: (files: FileRef[]) => void}} on
 */
function watchDrop(target, on) {
  let depth = 0;
  let inside = false;
  const isFiles = (/** @type {DragEvent} */ e) => !!e.dataTransfer && [...e.dataTransfer.types].includes('Files');
  const within = (/** @type {DragEvent} */ e) => e.target instanceof Node && target.contains(e.target);
  const hover = (/** @type {boolean} */ yes) => {
    if (yes === inside) return;
    inside = yes;
    on.over(yes);
  };
  const end = () => {
    depth = 0;
    hover(false);
    on.leave();
  };
  /** @type {[string, (e: DragEvent) => void][]} */
  const events = [
    ['dragenter', (e) => {
      if (!isFiles(e)) return;
      e.preventDefault();
      if (depth++ === 0) on.enter();
      hover(within(e));
    }],
    ['dragover', (e) => {
      if (!isFiles(e)) return;
      e.preventDefault();
      const yes = within(e);
      if (e.dataTransfer) e.dataTransfer.dropEffect = yes ? 'copy' : 'none';
      hover(yes);
    }],
    ['dragleave', (e) => {
      if (!isFiles(e) || --depth > 0) return;
      end();
    }],
    ['drop', (e) => {
      if (!isFiles(e)) return;
      e.preventDefault();
      const yes = within(e);
      end();
      if (yes) on.drop(refs(e.dataTransfer?.files ?? []));
    }],
  ];
  for (const [name, fn] of events) document.addEventListener(name, /** @type {any} */ (fn));
  return () => { for (const [name, fn] of events) document.removeEventListener(name, /** @type {any} */ (fn)); };
}

/**
 * 框里的缩略图：图片、视频交回一个临时地址和怎么松开它；别的是 `null`。
 * @param {FileRef} ref
 */
function preview(ref) {
  if (!/^(image|video)\//.test(ref.type)) return null;
  if (ref.stored) return { url: urls().blob(ref.stored.hash, ref.type), release: () => {} };
  // 本机的文件（`@` 选文件交过来的，只有路径）：照 `/media` 取本机文件
  if (ref.path && !(ref.file instanceof Blob)) return { url: urls().file(ref.path), release: () => {} };
  if (!(ref.file instanceof Blob)) return null;
  const url = URL.createObjectURL(ref.file);
  return { url, release: () => URL.revokeObjectURL(url) };
}

/**
 * 读文字文件的内容（框里的卡写有几行）：超过 `max` 字节的、读不了的交 `null`。
 * @param {FileRef} ref
 * @param {number} max
 */
async function text(ref, max) {
  if (ref.size > max) return null;
  try {
    if (ref.stored || (ref.path && !(ref.file instanceof Blob))) {
      // 票据要现换：等一次，拿不到就算读不了
      const key = ref.stored ? `blob:${ref.stored.hash}:text/plain:false:` : `file:${ref.path}:false`;
      const what = ref.stored ? { blob: ref.stored.hash, type: 'text/plain' } : { path: /** @type {string} */ (ref.path) };
      const url = ticketUrl(key) || (await mint(what).then((u) => { tickets.set(key, u); return u; }));
      if (!url) return null;
      const got = await fetch(url);
      return got.ok ? await got.text() : null;
    }
    return ref.file instanceof Blob ? await ref.file.text() : null;
  } catch { return null; }
}

/**
 * 读附件的一段字节（分块传给核心，`blob.write`，核心施工 W-5）：浏览器的文件切一段读出来。
 * @param {FileRef} ref
 * @param {number} offset
 * @param {number} length
 * @returns {Promise<Uint8Array>}
 */
async function read(ref, offset, length) {
  if (!(ref.file instanceof Blob)) throw new Error('没有内容');
  return new Uint8Array(await ref.file.slice(offset, offset + length).arrayBuffer());
}

/** 这台设备上存东西：`localStorage`（隐私窗口、清过数据时读写会抛，由内核的 `storage` 兜着）。 */
const store = /** @type {Store} */ ({
  getItem: (k) => localStorage.getItem(k),
  setItem: (k, v) => localStorage.setItem(k, v),
  removeItem: (k) => localStorage.removeItem(k),
});

/** 登录令牌作废了（核心回 `bad_login`）：清掉，交回要不要问人。 */
export function forgetLogin() {
  login = null;
  forgetTickets();
  try { localStorage.removeItem(LOGIN); } catch { /* 清不掉就算了 */ }
}

/** 起浏览器这个宿主。凭据交不出时也交得出一份宿主：`credentials()` 交 `null`，页面照着问人。 */
export function browserHost() {
  return {
    kind: 'browser',
    /** 开一条到核心的线（经网页软件的 `/ws`，不带凭据：凭据在握手里）。 */
    channel: () => /** @type {Channel} */ (/** @type {unknown} */ (new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`))),
    /**
     * 握手要带的凭据（`protocol.md`「握手」第 3 条：正好写一种）。三种都没有的交 `null`：页面问人要用户名和密码，
     * 拿到以后 [`usePassword`] 交回来。
     * @returns {Credentials|null}
     */
    credentials: () => {
      if (!login) login = storedLogin();
      if (!code) code = setupCode();
      if (login) return { login };
      if (code) return { code };
      if (asked) return { user: asked.user, password: asked.password };
      return null;
    },
    /** 一次性码设好了用户名密码：交回登录令牌，存起来、以后照它握手（W-8 的 `account.setup` 回的）。
     *
     * **一次性码一起丢掉**：它已经用过了，留着的话下一次握手会同时带 `code` 和 `login`，
     * 核心照「正好写一种」回 `bad_params`（施工 网页并进 真机撞见）。 */
    useLogin: (/** @type {string} */ token, /** @type {string|null} */ expires) => {
      login = token;
      code = null;
      asked = null;
      forgetTickets();
      try { localStorage.setItem(LOGIN, JSON.stringify({ token, expires })); } catch { /* 记不住就只管这一次 */ }
    },
    /** 问人拿到的用户名和密码：这一次握手先用它（核心用密码登录会回一个登录令牌）。 */
    usePassword: (/** @type {string} */ user, /** @type {string} */ password) => {
      asked = { user, password };
      forgetTickets();
    },
    /** 有没有要设密码的一次性码（`gqy web` 打出来的网址带的）。 */
    hasSetupCode: () => code !== null,
    /** 还能不能再连（`Connection` 断了重试时问一句）：并进以后没有口令这回事，永远交回「还能」。 */
    canReconnect: async () => true,
    /** 登录令牌作废了（核心回 `bad_login`）：清掉，以后问人。 */
    forgetLogin,
    urls: urls(),
    files: {
      pick,
      watchDrop,
      preview,
      refs,
      text,
      read,
    },
    /** 外面的链接：新标签页。 */
    open: (/** @type {string} */ url) => { window.open(url, '_blank', 'noopener'); },
    /** 接住页面里的链接：浏览器自己会办，什么都不做；交回怎么不接。 */
    intercept: (/** @type {HTMLElement} */ root) => () => {},
    clipboard: { write: (/** @type {string} */ text) => navigator.clipboard.writeText(text) },
    store,
  };
}

/** @typedef {NonNullable<ReturnType<typeof browserHost>>} Host 一个宿主交出的（桌面端那一份照这个样子） */
