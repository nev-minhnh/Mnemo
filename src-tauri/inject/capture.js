// Mnemo page script. Runs inside each app's page (WhatsApp Web, Messenger, Instagram,
// Telegram Web, Zalo Web). It never sends anything anywhere except to Mnemo itself,
// through Tauri's IPC:
//   * saves the messages and media the page shows into the local archive (level A),
//   * tells Mnemo which conversation is open (for the AI panel),
//   * shows the writing-assistant button next to the text cursor.
// Page settings (selectors, CSS) come from window.__MNEMO__.cfg.
(function () {
  'use strict';
  const M = window.__MNEMO__;
  if (!M || window.__MNEMO_STARTED__) return;
  window.__MNEMO_STARTED__ = true;
  const cfg = M.cfg || {};

  // ---------- IPC ----------
  function invoke(cmd, args) {
    const ipc = window.__TAURI_INTERNALS__;
    if (!ipc || typeof ipc.invoke !== 'function') return Promise.reject(new Error('Mnemo IPC unavailable'));
    return ipc.invoke(cmd, args || {});
  }
  let logBudget = 40;
  function log(msg) {
    if (logBudget-- <= 0) return;
    invoke('mnemo_log', { message: String(msg).slice(0, 500) }).catch(() => {});
  }

  // ---------- helpers ----------
  function fnv(str, seed) {
    let h = seed >>> 0;
    for (let i = 0; i < str.length; i++) {
      h ^= str.charCodeAt(i);
      h = Math.imul(h, 16777619) >>> 0;
    }
    return ('0000000' + h.toString(16)).slice(-8);
  }
  const hash = (s) => fnv(s, 2166136261) + fnv(s, 3141592653);
  function q(root, sel) {
    if (!sel) return null;
    try { return root.querySelector(sel); } catch (e) { return null; }
  }
  function qa(root, sel) {
    if (!sel) return [];
    try { return Array.from(root.querySelectorAll(sel)); } catch (e) { return []; }
  }
  function textOf(node, exclude) {
    if (!node) return '';
    if (exclude && exclude.length) {
      const clone = node.cloneNode(true);
      exclude.forEach((s) => qa(clone, s).forEach((n) => n.remove()));
      return (clone.innerText || clone.textContent || '').trim();
    }
    return (node.innerText || node.textContent || '').trim();
  }
  // A field spec: {sel, attr ("text" by default), re (first group), closest, exclude}
  function field(el, spec) {
    if (!spec) return '';
    const specs = Array.isArray(spec) ? spec : [spec];
    for (const s of specs) {
      let node = el;
      if (s.closest) node = el.closest(s.closest);
      if (node && s.sel) node = q(node, s.sel);
      if (!node) continue;
      let v = !s.attr || s.attr === 'text' ? textOf(node, s.exclude) : node.getAttribute(s.attr) || '';
      if (s.re && v) {
        const m = new RegExp(s.re).exec(v);
        v = m ? (m[1] !== undefined ? m[1] : m[0]) : '';
      }
      v = (v || '').trim();
      if (v) return v;
    }
    return '';
  }

  // ---------- notifications: hand them to Mnemo instead of showing them ----------
  // The page believes notifications are allowed; every one it creates goes to Mnemo,
  // which shows it in its own corner window. A click there replays the page's own click,
  // so the app opens the right conversation and message.
  const pageNotices = new Map();
  let noticeSeq = 0;
  function safeData(d) {
    try {
      const s = JSON.stringify(d);
      return s && s.length < 1500 ? JSON.parse(s) : null;
    } catch (e) { return null; }
  }
  function forwardNotice(title, options, extra) {
    const id = 'n' + (++noticeSeq) + '-' + Date.now();
    const opts = options || {};
    invoke('mnemo_notify', {
      notice: { id: id, title: String(title || '').slice(0, 300), body: String(opts.body || '').slice(0, 2000), extra: extra || { tag: opts.tag || null, data: safeData(opts.data) } },
    }).catch(() => {});
    return id;
  }
  class MnemoNotification extends EventTarget {
    constructor(title, options) {
      super();
      const o = options || {};
      this.title = String(title || '');
      this.body = o.body || '';
      this.tag = o.tag || '';
      this.data = o.data;
      this.icon = o.icon || '';
      this.onclick = null; this.onclose = null; this.onshow = null; this.onerror = null;
      this._id = forwardNotice(title, o);
      pageNotices.set(this._id, this);
      if (pageNotices.size > 300) pageNotices.delete(pageNotices.keys().next().value);
      setTimeout(() => {
        const ev = new Event('show');
        try { this.dispatchEvent(ev); if (typeof this.onshow === 'function') this.onshow(ev); } catch (e) {}
      }, 0);
    }
    close() {
      pageNotices.delete(this._id);
      const ev = new Event('close');
      try { this.dispatchEvent(ev); if (typeof this.onclose === 'function') this.onclose(ev); } catch (e) {}
    }
    static get permission() { return 'granted'; }
    static requestPermission(cb) { if (typeof cb === 'function') cb('granted'); return Promise.resolve('granted'); }
    static get maxActions() { return 2; }
  }
  try { Object.defineProperty(window, 'Notification', { value: MnemoNotification, writable: true, configurable: true }); } catch (e) {}
  try {
    if (navigator.permissions && navigator.permissions.query) {
      const query = navigator.permissions.query.bind(navigator.permissions);
      navigator.permissions.query = (d) => (d && d.name === 'notifications')
        ? Promise.resolve({ name: 'notifications', state: 'granted', status: 'granted', onchange: null, addEventListener() {}, removeEventListener() {} })
        : query(d);
    }
  } catch (e) {}
  try {
    if (window.ServiceWorkerRegistration) {
      ServiceWorkerRegistration.prototype.showNotification = function (title, options) {
        const o = options || {};
        forwardNotice(title, o, { tag: o.tag || null, data: safeData(o.data) });
        return Promise.resolve();
      };
      ServiceWorkerRegistration.prototype.getNotifications = function () { return Promise.resolve([]); };
    }
    // Telegram Web asks its service worker to show message notifications.
    if (window.ServiceWorker) {
      const post = ServiceWorker.prototype.postMessage;
      ServiceWorker.prototype.postMessage = function (msg) {
        try {
          if (msg && msg.type === 'showMessageNotification' && msg.payload) {
            const p = msg.payload;
            forwardNotice(p.title, { body: p.body }, { chatId: p.chatId != null ? String(p.chatId) : null, messageId: p.messageId != null ? String(p.messageId) : null });
            return;
          }
        } catch (e) {}
        return post.apply(this, arguments);
      };
    }
  } catch (e) {}

  function clickLikeAUser(el) {
    const target = el.closest('a, [role="listitem"], [role="row"], [role="button"], [role="link"], li, [tabindex]') || el;
    ['mousedown', 'mouseup', 'click'].forEach((type) => {
      try { target.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, view: window })); } catch (e) {}
    });
  }
  function openChatByTitle(title) {
    const t = (title || '').trim();
    if (!t) return false;
    const sel = (cfg.chatList && cfg.chatList.sel) || '';
    const candidates = qa(document, sel).concat(qa(document, '[title]'));
    for (const el of candidates) {
      const own = (el.getAttribute('title') || '').trim();
      const text = ((el.innerText || '').trim().split('\n')[0] || '').trim();
      if (own === t || text === t) { clickLikeAUser(el); return true; }
    }
    return false;
  }
  function focusMessage(key) {
    if (!key) return;
    let tries = 0;
    const find = () => {
      const k = String(key).replace(/"/g, '');
      const el = q(document, `[data-message-id="${k}"]`) || q(document, `[data-id$="_${k}"]`) || q(document, `[data-id="${k}"]`) || document.getElementById('message-' + k);
      if (el) {
        el.scrollIntoView({ block: 'center' });
        const old = el.style.outline;
        el.style.outline = '2px solid #8b5cf6';
        setTimeout(() => { el.style.outline = old; }, 2500);
      } else if (++tries < 15) setTimeout(find, 400);
    };
    find();
  }
  window.__MNEMO_NOTICE_CLICK__ = function (p) {
    p = p || {};
    const n = p.pageId && pageNotices.get(p.pageId);
    if (n) {
      const ev = new Event('click', { cancelable: true });
      try { n.dispatchEvent(ev); } catch (e) {}
      try { if (typeof n.onclick === 'function') n.onclick.call(n, ev); } catch (e) {}
      pageNotices.delete(p.pageId);
      return;
    }
    const x = p.extra || {};
    if (x.chatId) {
      location.hash = '#' + x.chatId;
      if (x.messageId) setTimeout(() => focusMessage(x.messageId), 600);
      return;
    }
    openChatByTitle(p.title);
  };

  // ---------- the signed-in person's avatar ----------
  let avatarSent = '';
  async function detectAvatar() {
    const specs = cfg.selfAvatar || [];
    for (const s of specs) {
      // "last": the bottom-most match (WhatsApp keeps your own photo at the end of its rail);
      // "not": skip matches inside these elements (Meta AI, channels…).
      let nodes = qa(document, s.sel);
      if (s.not) nodes = nodes.filter((n) => !n.closest(s.not));
      const node = s.last ? nodes[nodes.length - 1] : nodes[0];
      if (!node) continue;
      const src = s.attr ? (node.getAttribute(s.attr) || '') : (node.currentSrc || node.src || '');
      if (!src || src === avatarSent) return;
      avatarSent = src;
      try {
        if (src.startsWith('https:') && new URL(src).origin !== location.origin) {
          await invoke('mnemo_profile', { avatarUrl: src });
          return;
        }
        const blob = await (await fetch(src)).blob();
        const bmp = await createImageBitmap(blob);
        const size = 96;
        const canvas = document.createElement('canvas');
        canvas.width = size; canvas.height = size;
        const ctx = canvas.getContext('2d');
        const k = Math.max(size / bmp.width, size / bmp.height);
        ctx.drawImage(bmp, (size - bmp.width * k) / 2, (size - bmp.height * k) / 2, bmp.width * k, bmp.height * k);
        await invoke('mnemo_profile', { avatarData: canvas.toDataURL('image/png') });
      } catch (e) {
        avatarSent = '';
      }
      return;
    }
  }
  setTimeout(detectAvatar, 6000);
  // Often: some apps show your photo only on one screen (Telegram: Settings).
  setInterval(detectAvatar, 5000);

  // Names of the conversations the chat list marks unread (for "From: 3 người" notices).
  let lastSenders = '';
  // "17.6K" → 17600, "1,234" → 1234, "99+" → 99.
  function parseCount(t) {
    const m = /([\d.,]+)\s*([KkMm])?/.exec(t || '');
    if (!m) return 0;
    let n = parseFloat(m[1].replace(/,(?=\d{3}\b)/g, '').replace(',', '.'));
    if (!isFinite(n)) return 0;
    if (m[2]) n *= /k/i.test(m[2]) ? 1000 : 1000000;
    return Math.round(n);
  }
  // The unread-message count, read from the chat list's badges (cfg.unreadCount).
  function unreadCount() {
    const specs = cfg.unreadCount ? [].concat(cfg.unreadCount) : [];
    for (const s of specs) {
      let nodes = qa(document, s.sel);
      if (s.not) nodes = nodes.filter((n) => !n.closest(s.not) && !n.matches(s.not));
      if (s.mode === 'first') { if (nodes.length) return parseCount(nodes[0].textContent); continue; }
      // Sum: an empty list is a real zero once the chat list itself is on the page.
      if (!nodes.length && !(s.listSel && q(document, s.listSel))) continue;
      return nodes.reduce((sum, n) => sum + (parseCount(n.textContent) || (s.zeroIsOne ? 1 : 0)), 0);
    }
    return null;
  }
  function unreadSenders() {
    const u = cfg.unreadChats;
    if (!u || !u.row) return;
    const names = [];
    for (const row of qa(document, u.row)) {
      const marked = u.badge ? (row.matches(u.badge) || !!q(row, u.badge)) : true;
      if (!marked) continue;
      const n = field(row, u.name || { attr: 'text' }).split('\n')[0].trim();
      if (n && names.indexOf(n) === -1) names.push(n);
      if (names.length >= 200) break;
    }
    const count = unreadCount();
    const sig = names.join('\u0001') + '|' + count;
    if (sig === lastSenders) return;
    lastSenders = sig;
    invoke('mnemo_senders', { names, count }).catch(() => {});
  }
  setInterval(unreadSenders, 2000);

  // A page header Mnemo keeps above the window (cfg.cropTop): its height, on the paths given.
  let cropSent = -1;
  function cropTop() {
    const c = cfg.cropTop;
    if (!c) return;
    const onPath = !c.paths || c.paths.some((p) => location.pathname === p || location.pathname.startsWith(p + '/'));
    const bar = onPath ? q(document, c.sel) : null;
    let h = 0;
    if (bar) {
      const r = bar.getBoundingClientRect();
      if (r.top <= 1 && r.height > 20 && r.height < 120) h = Math.round(r.bottom);
    }
    if (h === cropSent) return;
    cropSent = h;
    invoke('mnemo_crop', { top: h }).catch(() => {});
  }
  setInterval(cropTop, 1000);

  // ---------- styles (constructable sheets are allowed by the pages' CSP) ----------
  let pageSheet = null;
  function setPageCss(on) {
    if (!cfg.css) return;
    try {
      if (on && !pageSheet) {
        pageSheet = new CSSStyleSheet();
        pageSheet.replaceSync(cfg.css);
        document.adoptedStyleSheets = [...document.adoptedStyleSheets, pageSheet];
      } else if (!on && pageSheet) {
        document.adoptedStyleSheets = document.adoptedStyleSheets.filter((s) => s !== pageSheet);
        pageSheet = null;
      }
    } catch (e) { log('css: ' + e); }
  }

  // ---------- runtime settings from Mnemo ----------
  let runtime = { capture: false, rules: null, autoCheck: false, assist: true, hotkey: 'Ctrl+Shift+Space' };
  function refreshRuntime() {
    return invoke('mnemo_config').then((r) => { runtime = Object.assign(runtime, r || {}); }).catch(() => {});
  }
  refreshRuntime();
  setInterval(refreshRuntime, 30000);

  // ---------- conversation ----------
  function firstMessageKeySource() {
    const c = cfg.chat && cfg.chat.key;
    if (!c || c.from !== 'message') return '';
    const el = q(document, (cfg.message && cfg.message.sel) || '');
    if (!el) return '';
    const v = el.getAttribute(c.attr) || '';
    const m = c.re ? new RegExp(c.re).exec(v) : null;
    return m ? (m[1] || m[0]) : '';
  }
  function chatKey() {
    const c = (cfg.chat && cfg.chat.key) || {};
    let v = '';
    if (c.from === 'hash') v = location.hash;
    else if (c.from === 'path') v = location.pathname;
    else if (c.from === 'message') return firstMessageKeySource();
    else if (c.from === 'dom') return field(document, c) ? 't:' + field(document, c) : '';
    if (!c.re) return v;
    const m = new RegExp(c.re).exec(v);
    return m ? (m[1] !== undefined ? m[1] : m[0]) : '';
  }
  function chatTitle() {
    return field(document, cfg.chat && cfg.chat.title).slice(0, 200);
  }
  function isGroup(key) {
    const re = cfg.chat && cfg.chat.groupKeyRe;
    if (re && new RegExp(re).test(key)) return true;
    const gs = cfg.chat && cfg.chat.groupSel;
    return !!(gs && q(document, gs));
  }

  let lastContext = '';
  function reportContext() {
    const key = chatKey();
    const title = key ? chatTitle() : '';
    const sig = key + '|' + title;
    if (sig === lastContext) return;
    lastContext = sig;
    invoke('mnemo_context', { chatKey: key || null, title: title || null, isGroup: key ? isGroup(key) : false }).catch(() => {});
  }

  // ---------- route loop: redirects, CSS, conversation changes ----------
  function route() {
    const p = location.pathname;
    const hr = cfg.homeRedirect;
    if (hr && Array.isArray(hr.from) && hr.from.includes(p) && hr.to) {
      location.replace(hr.to);
      return;
    }
    const paths = cfg.cssPaths;
    setPageCss(!paths || paths.some((x) => p.startsWith(x)));
    reportContext();
  }
  setInterval(route, 800);
  document.addEventListener('DOMContentLoaded', route);

  // ---------- capture ----------
  const sent = new Map(); // full key -> signature
  const mediaSent = new Set();
  let noMatchSince = 0;
  let lastReadUpTo = null;
  let scanTimer = null;

  function parseWhatsAppTime(s) {
    // "[10:30, 9/11/2026]" or "[10:30 PM, 11/9/2026]" or "[10:30, 2026-11-09]"
    const m = /\[(\d{1,2}):(\d{2})(?:\s*([AaPp])\.?\s*[Mm]\.?)?,\s*(\d{1,4})[\/.\-](\d{1,2})[\/.\-](\d{1,4})\]/.exec(s || '');
    if (!m) return null;
    let h = +m[1];
    const min = +m[2];
    if (m[3]) {
      const pm = m[3].toLowerCase() === 'p';
      if (pm && h < 12) h += 12;
      if (!pm && h === 12) h = 0;
    }
    let a = +m[4], b = +m[5], c = +m[6];
    let y, mo, d;
    if (m[4].length === 4) { y = a; mo = b; d = c; }
    else {
      y = c < 100 ? 2000 + c : c;
      let mdy;
      if (a > 12) mdy = false;
      else if (b > 12) mdy = true;
      else mdy = /^en-US/i.test(navigator.language || '');
      if (mdy) { mo = a; d = b; } else { d = a; mo = b; }
    }
    const dt = new Date(y, mo - 1, d, h, min, 0, 0);
    return isNaN(dt.getTime()) ? null : dt.getTime();
  }

  function outgoingOf(el, key) {
    const o = cfg.message && cfg.message.outgoing;
    if (!o) return null;
    if (o.keyRe) return new RegExp(o.keyRe).test(key);
    if (o.selfClass) return el.classList.contains(o.selfClass);
    if (o.closest) return !!el.closest(o.closest) || !!q(el, o.closest);
    if (o.sel) return !!q(el, o.sel);
    return null;
  }

  function extract(el, prevText, occurrences, ctxKey, lastSender) {
    const mc = cfg.message || {};
    const text = field(el, mc.text).slice(0, 50000);
    let sender = field(el, mc.sender).slice(0, 200);
    if (!sender && mc.inheritSender) sender = lastSender || '';
    const timeText = field(el, mc.time).slice(0, 80);
    let rawKey = mc.key ? field(el, mc.key) : '';
    if (!rawKey) {
      const base = ctxKey + '|' + sender + '|' + text + '|' + prevText;
      const n = (occurrences.get(base) || 0) + 1;
      occurrences.set(base, n);
      rawKey = 'h:' + hash(base + '|' + n);
    }
    const media = qa(el, (cfg.media && cfg.media.sel) || '');
    if (!text && !media.length) return null;
    let ts = null;
    if (mc.timeParse === 'whatsapp') ts = parseWhatsAppTime(field(el, mc.timeSource || mc.time));
    if (mc.timeParse === 'epoch') {
      const t = parseFloat(timeText);
      ts = isFinite(t) && t > 1e9 ? Math.round(t < 1e11 ? t * 1000 : t) : null;
    }
    if (mc.timeParse === 'iso') {
      const t = Date.parse(timeText);
      ts = isNaN(t) ? null : t;
    }
    const recalled = !!(text && (mc.recalled || []).some((p) => text.indexOf(p) !== -1));
    return {
      key: rawKey.slice(0, 400),
      sender: sender || null,
      outgoing: outgoingOf(el, rawKey),
      ts: ts,
      timeText: timeText || null,
      text: recalled ? '' : text,
      kind: text ? (media.length ? 'text+media' : 'text') : 'media',
      recalled: recalled,
      media: media,
    };
  }

  function scan() {
    scanTimer = null;
    if (!runtime.capture) return;
    const key = chatKey();
    if (!key) return;
    const els = qa(document, (cfg.message && cfg.message.sel) || '');
    if (!els.length) {
      if (!noMatchSince) noMatchSince = Date.now();
      else if (Date.now() - noMatchSince > 60000) {
        log('no message matched selector "' + (cfg.message && cfg.message.sel) + '" in chat ' + key);
        noMatchSince = Date.now() + 3600000;
      }
      return;
    }
    noMatchSince = 0;
    const occurrences = new Map();
    const all = [];
    let prevText = '';
    let lastSender = '';
    for (const el of els) {
      const r = extract(el, prevText, occurrences, key, lastSender);
      if (!r) continue;
      r.el = el;
      all.push(r);
      prevText = r.text.slice(0, 200);
      if (r.sender) lastSender = r.sender;
    }
    // Messages above the app's "unread" divider have been seen.
    let readUpToKey = null;
    const divider = cfg.unreadDivider && q(document, cfg.unreadDivider);
    if (divider) {
      for (const r of all) {
        if (r.el.compareDocumentPosition(divider) & Node.DOCUMENT_POSITION_FOLLOWING) readUpToKey = r.key;
      }
    }
    const batch = [];
    all.forEach((r, i) => {
      const sig = r.text + '|' + r.recalled + '|' + (r.sender || '');
      const full = key + '/' + r.key;
      if (sent.get(full) === sig) return;
      sent.set(full, sig);
      batch.push({
        key: r.key, sender: r.sender, outgoing: r.outgoing, ts: r.ts, timeText: r.timeText,
        text: r.text, kind: r.kind, recalled: r.recalled,
        prevKey: i > 0 ? all[i - 1].key : null,
        nextKey: i + 1 < all.length ? all[i + 1].key : null,
      });
    });
    const send = (items) => invoke('mnemo_ingest', {
      batch: { chatKey: key, title: chatTitle() || null, isGroup: isGroup(key), readUpToKey: readUpToKey, messages: items },
    });
    const chunks = [];
    for (let i = 0; i < batch.length; i += 100) chunks.push(batch.slice(i, i + 100));
    let p = Promise.resolve();
    chunks.forEach((c) => { p = p.then(() => send(c)); });
    if (!chunks.length && readUpToKey && readUpToKey !== lastReadUpTo) p = send([]);
    if (readUpToKey) lastReadUpTo = readUpToKey;
    p.then(() => captureMedia(key, all)).catch((e) => {
      // Let the next scan retry what didn't arrive.
      batch.forEach((b) => sent.delete(key + '/' + b.key));
      log('ingest failed: ' + e);
    });
  }

  function mediaKind(node) {
    const tag = node.tagName;
    if (tag === 'IMG') return 'image';
    if (tag === 'VIDEO') return 'video';
    if (tag === 'AUDIO') return 'audio';
    return 'document';
  }
  function allowedKind(kind) {
    const r = runtime.rules;
    if (!r || !r.enabled) return false;
    if (kind === 'image') return r.images;
    if (kind === 'video') return r.videos;
    if (kind === 'audio') return r.audio;
    return r.documents || r.other;
  }
  function toBase64(buf) {
    const bytes = new Uint8Array(buf);
    let bin = '';
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(bin);
  }

  async function captureMedia(chatKeyNow, records) {
    const r = runtime.rules;
    if (!r || !r.enabled) return;
    const minSize = (cfg.media && cfg.media.minSize) || 60;
    for (const rec of records) {
      for (const node of rec.media) {
        const src = node.currentSrc || node.src || node.getAttribute('href') || '';
        if (!src) continue;
        const kind = mediaKind(node);
        if (!allowedKind(kind)) continue;
        if (kind === 'image' && (node.naturalWidth || 0) < minSize && (node.naturalHeight || 0) < minSize) continue;
        const id = chatKeyNow + '/' + rec.key + '/' + src;
        if (mediaSent.has(id)) continue;
        mediaSent.add(id);
        const meta = { chatKey: chatKeyNow, msgKey: rec.key, kind: kind, name: node.getAttribute('download') || node.getAttribute('alt') || '', mime: '' };
        try {
          if (src.startsWith('blob:')) {
            const res = await fetch(src);
            const blob = await res.blob();
            if (blob.size > (r.maxBytes || 26214400)) continue;
            meta.mime = blob.type || '';
            const data = toBase64(await blob.arrayBuffer());
            await invoke('mnemo_media_blob', { meta: meta, data: data });
          } else if (src.startsWith('https:')) {
            await invoke('mnemo_media_url', { meta: meta, url: src });
          }
        } catch (e) {
          mediaSent.delete(id);
          log('media failed: ' + e);
        }
      }
    }
  }

  function schedule() {
    if (scanTimer) return;
    scanTimer = setTimeout(scan, 1200);
  }
  function startObserver() {
    if (!document.body) { setTimeout(startObserver, 500); return; }
    new MutationObserver(schedule).observe(document.body, { childList: true, subtree: true, characterData: true });
    schedule();
  }
  startObserver();

  // ---------- writing assistant ----------
  let host = null, shadow = null, badge = null, lastEditable = null, idleTimer = null, lastChecked = '';
  function isEditable(el) {
    if (!el || el === host) return false;
    if (el.tagName === 'TEXTAREA') return true;
    if (el.tagName === 'INPUT') return /^(text|search)$/i.test(el.type || 'text');
    return !!el.isContentEditable;
  }
  function editableRoot(el) {
    if (!el) return null;
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') return el;
    let n = el;
    while (n.parentElement && n.parentElement.isContentEditable) n = n.parentElement;
    return n;
  }
  function editableText(el) {
    if (!el) return '';
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') return el.value || '';
    return (el.innerText || '').replace(/ /g, ' ').replace(/\n$/, '');
  }
  function caretRect(el) {
    try {
      const sel = window.getSelection();
      if (sel && sel.rangeCount && el.contains(sel.anchorNode)) {
        const range = sel.getRangeAt(0).cloneRange();
        range.collapse(false);
        const rects = range.getClientRects();
        const r = rects.length ? rects[rects.length - 1] : range.getBoundingClientRect();
        if (r && (r.width || r.height || r.x || r.y)) return r;
      }
    } catch (e) {}
    return el.getBoundingClientRect();
  }
  function ensureBadge() {
    if (badge) return;
    host = document.createElement('mnemo-assist');
    host.style.cssText = 'all:initial;position:fixed;z-index:2147483647;left:0;top:0;width:0;height:0;';
    shadow = host.attachShadow({ mode: 'closed' });
    try {
      const sheet = new CSSStyleSheet();
      sheet.replaceSync(`
        button{all:unset;box-sizing:border-box;position:fixed;display:none;align-items:center;justify-content:center;
          width:26px;height:26px;border-radius:13px;cursor:pointer;font:600 12px/1 "Segoe UI",system-ui,sans-serif;
          color:#fff;background:linear-gradient(135deg,#5b5bf7,#9b5cf6);box-shadow:0 2px 8px rgba(30,20,90,.35);}
        button:hover{transform:scale(1.08)}
        button.busy{opacity:.7}
        button.issues{background:linear-gradient(135deg,#e5484d,#f76b15)}
        button.clean{background:linear-gradient(135deg,#12a594,#30a46c)}`);
      shadow.adoptedStyleSheets = [sheet];
    } catch (e) {}
    badge = document.createElement('button');
    badge.title = 'Mnemo – trợ lý viết (' + (runtime.hotkey || 'Ctrl+Shift+Space') + ')';
    badge.textContent = 'M';
    badge.addEventListener('mousedown', (e) => { e.preventDefault(); e.stopPropagation(); });
    badge.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); openAssist(); });
    shadow.appendChild(badge);
    document.documentElement.appendChild(host);
  }
  function placeBadge(el) {
    ensureBadge();
    const r = caretRect(el);
    const x = Math.min(window.innerWidth - 32, Math.max(4, r.right + 8));
    const y = Math.min(window.innerHeight - 32, Math.max(4, r.top - 30));
    badge.style.left = x + 'px';
    badge.style.top = y + 'px';
    badge.style.display = 'flex';
  }
  function hideBadge() { if (badge) { badge.style.display = 'none'; badge.className = ''; badge.textContent = 'M'; } }

  function openAssist() {
    const el = lastEditable;
    if (!el) return;
    const text = editableText(el);
    if (!text.trim()) return;
    const r = caretRect(el);
    hideBadge();
    invoke('mnemo_assist_open', {
      text: text.slice(0, 20000),
      rect: { x: r.left, y: r.top, width: r.width, height: r.height },
      viewport: { width: window.innerWidth, height: window.innerHeight },
    }).catch((e) => log('assist: ' + e));
  }

  function onTyping() {
    const el = editableRoot(document.activeElement);
    if (!isEditable(el)) { return; }
    lastEditable = el;
    hideBadge();
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      if (!runtime.assist) return;
      const text = editableText(el);
      if (text.trim().length < 3 || document.activeElement !== el && !el.contains(document.activeElement)) return;
      placeBadge(el);
      if (runtime.autoCheck && text !== lastChecked) {
        lastChecked = text;
        badge.className = 'busy';
        badge.textContent = '…';
        invoke('mnemo_assist_check', { text: text.slice(0, 20000) }).then((res) => {
          if (editableText(el) !== text) return;
          const n = (res && res.issues) || 0;
          badge.className = n ? 'issues' : 'clean';
          badge.textContent = n ? String(Math.min(n, 99)) : '✓';
        }).catch(() => { badge.className = ''; badge.textContent = 'M'; });
      }
    }, 1200);
  }
  document.addEventListener('input', onTyping, true);
  document.addEventListener('focusout', () => setTimeout(() => {
    const a = document.activeElement;
    if (!lastEditable || (a !== lastEditable && !lastEditable.contains(a))) hideBadge();
  }, 250), true);
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.shiftKey && (e.code === 'Space' || e.key === ' ')) {
      const el = editableRoot(document.activeElement);
      if (isEditable(el)) { lastEditable = el; e.preventDefault(); e.stopPropagation(); openAssist(); }
    }
  }, true);

  function composeTarget() {
    if (lastEditable && document.contains(lastEditable)) return lastEditable;
    const sel = cfg.compose && cfg.compose.sel;
    const el = q(document, sel || '[contenteditable="true"][role="textbox"], footer [contenteditable="true"]');
    return el ? editableRoot(el) : null;
  }

  // Called by Mnemo: replace the whole draft ("replace") or insert at the cursor ("insert").
  window.__MNEMO_APPLY__ = function (text, mode) {
    const el = composeTarget();
    if (!el) return false;
    el.focus();
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') {
      if (mode === 'insert') el.setRangeText(text, el.selectionStart || 0, el.selectionEnd || 0, 'end');
      else el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      return true;
    }
    const sel = window.getSelection();
    if (mode !== 'insert') {
      const range = document.createRange();
      range.selectNodeContents(el);
      sel.removeAllRanges();
      sel.addRange(range);
    } else if (!sel.rangeCount || !el.contains(sel.anchorNode)) {
      const range = document.createRange();
      range.selectNodeContents(el);
      range.collapse(false);
      sel.removeAllRanges();
      sel.addRange(range);
    }
    const ok = document.execCommand('insertText', false, text);
    if (!ok) {
      el.textContent = text;
      el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }));
    }
    lastChecked = '';
    return true;
  };
})();
