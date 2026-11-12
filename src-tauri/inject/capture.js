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
  // Two buttons by the cursor in the message box: "M" opens the AI writing popup (rewrite,
  // ask), the round checker checks the draft and opens the Writing assistant panel. Only
  // the draft in the box is ever read or sent: never the conversation or anything else.
  let host = null, shadow = null, pair = null, badge = null, checker = null;
  let lastEditable = null, idleTimer = null, lastChecked = '';
  let marks = null, glow = null, card = null, cardTimer = null, shownIssue = null;
  let issues = [], issuesEl = null, result = null, busy = false, checkSeq = 0;
  const CAT_COLOR = { correctness: '#f03c45', clarity: '#1a8cf8', engagement: '#2bb86a', delivery: '#8b5cf6' };
  const CATS = ['correctness', 'clarity', 'engagement', 'delivery'];
  const CAT_LABEL = { correctness: 'Đúng', clarity: 'Rõ ràng', engagement: 'Hấp dẫn', delivery: 'Giọng điệu' };
  let lastCount = 0;
  const SVG_CHECK = '<svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 17.5 7.6 6.5 11.2 17.5M5.3 13.6h4.6"/><path d="m13.5 13.8 2.6 2.7 4.7-6"/></svg>';
  const esc2 = (s) => String(s == null ? '' : s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

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
    if (pair) return;
    host = document.createElement('mnemo-assist');
    host.style.cssText = 'all:initial;position:fixed;z-index:2147483647;left:0;top:0;width:0;height:0;';
    shadow = host.attachShadow({ mode: 'closed' });
    try {
      const sheet = new CSSStyleSheet();
      sheet.replaceSync(`
        .pair{position:fixed;display:none;gap:6px;align-items:center;transition:opacity .16s ease,transform .16s ease;opacity:0;transform:translateY(4px)}
        .pair.on{display:flex}.pair.in{opacity:1;transform:none}
        .b{all:unset;box-sizing:border-box;position:relative;display:flex;align-items:center;justify-content:center;width:28px;height:28px;border-radius:14px;
          cursor:pointer;font:700 12px/1 "Segoe UI",system-ui,sans-serif;color:#fff;transition:transform .12s ease,box-shadow .12s ease}
        .b:hover{transform:scale(1.08)}
        .m{background:linear-gradient(135deg,#19d3ff,#2f6bff 40%,#9b4dff 70%,#ff5a9e);box-shadow:0 2px 10px rgba(80,60,200,.4)}
        .c{background:#12151c;box-shadow:0 2px 8px rgba(0,0,0,.35)}
        .c::before{content:"";position:absolute;inset:0;border-radius:50%;padding:2.5px;background:var(--ring,conic-gradient(#19d3ff 0 100%));
          -webkit-mask:linear-gradient(#000 0 0) content-box,linear-gradient(#000 0 0);-webkit-mask-composite:xor;mask-composite:exclude}
        .c.scan::before{background:conic-gradient(from 0deg,#f03c45 0 62%,#ffb84a 62% 100%);animation:spin .9s linear infinite}
        .c.clean{color:#3fd29b}.c.clean::before{background:#3fd29b}
        .c.err{color:#ffab4a}.c.err::before{background:#ffab4a}
        .c span{position:relative}
        @keyframes spin{to{transform:rotate(360deg)}}
        .ul{position:fixed;height:2px;border-radius:1px;pointer-events:none;transition:opacity .15s}
        .hl{position:fixed;border-radius:3px;pointer-events:none;animation:hl 1.6s ease-out forwards}
        @keyframes hl{0%{opacity:0;transform:scale(1.08)}15%{opacity:1;transform:none}75%{opacity:1}100%{opacity:.35}}
.card{position:fixed;display:none;width:340px;max-width:calc(100vw - 16px);box-sizing:border-box;padding:12px 14px;border-radius:14px;
          font:13px/1.45 "Segoe UI",system-ui,sans-serif;color:#eef1f7;background:#12151c;border:1px solid #252b39;
          box-shadow:0 12px 34px rgba(0,0,0,.5);pointer-events:auto;opacity:0;transform:translateY(4px);transition:opacity .14s ease,transform .14s ease;
          overflow-wrap:anywhere;word-break:break-word;white-space:normal}
        .card.in{opacity:1;transform:none}
        .card .t{display:flex;align-items:center;gap:6px;font-size:12.5px;font-weight:600;margin-bottom:6px}
        .card .t i{font-style:normal;font-weight:400;color:#8a92a6}
        .card .d{display:inline-block;width:9px;height:9px;border-radius:50%;flex:none;background:var(--c)}
        .card .t b{color:var(--c)}
        .card .h{font-size:14px;margin-bottom:6px}
        .card del{display:block;color:#ff5a6e;margin-bottom:6px}
        .card .fix{all:unset;display:block;box-sizing:border-box;max-width:100%;cursor:pointer;background:#2bb86a;color:#fff;font-weight:600;
          border-radius:8px;padding:6px 12px;overflow-wrap:anywhere;transition:filter .12s}
        .card .fix:hover{filter:brightness(1.1)}
        .card .x{color:#c3c9d6;font-size:12.5px;margin-top:8px}
        .card .a{display:flex;gap:6px;margin-top:10px;flex-wrap:wrap}
        .card .a span{all:unset;cursor:pointer;padding:5px 12px;border-radius:8px;font-size:12.5px;border:1px solid #2c3446;color:#eef1f7}
        .card .a span:hover{background:#222836}`);
      shadow.adoptedStyleSheets = [sheet];
    } catch (e) {}
    marks = document.createElement('div');
    glow = document.createElement('div');
    pair = document.createElement('div');
    pair.className = 'pair';
    badge = document.createElement('button');
    badge.className = 'b m';
    badge.innerHTML = '<svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><path d="M12 2.5l1.9 5.6 5.6 1.9-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.9z"/><path d="M18.5 15l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z" opacity=".8"/></svg>';
    badge.title = 'Mnemo – trợ lý AI: viết lại, hỏi AI (' + (runtime.hotkey || 'Ctrl+Shift+Space') + ')';
    checker = document.createElement('button');
    checker.className = 'b c';
    checker.innerHTML = '<span>' + SVG_CHECK + '</span>';
    checker.title = 'Kiểm tra chính tả, ngữ pháp, văn phong';
    [badge, checker].forEach((b) => b.addEventListener('mousedown', (e) => { e.preventDefault(); e.stopPropagation(); }));
    badge.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); openAssist(); });
    checker.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); openChecker(); });
    pair.append(checker);
    card = document.createElement('div');
    card.className = 'card';
    card.addEventListener('mousedown', (e) => { e.preventDefault(); e.stopPropagation(); });
    card.addEventListener('mouseleave', () => hideCard(400));
    card.addEventListener('mouseenter', () => clearTimeout(cardTimer));
    shadow.append(marks, glow, pair, card);
    document.documentElement.appendChild(host);
  }
  // The Writing assistant lives in Mnemo's right-hand panel (its button sits beside the AI
  // assistant's in the Mnemo bar); the page only draws underlines and hover cards.
  // The checker sits in the bottom-right corner of the message box (as Grammarly's does);
  // clicking it opens the Writing tab of Mnemo's AI panel. Everything else is in that panel.
  let badgeEl = null, checkError = '';
  function placeBadge(el) {
    ensureBadge();
    if (!el || !document.contains(el)) { hideBadge(); return; }
    const r = el.getBoundingClientRect();
    if (r.width < 40 || r.height < 12 || !editableText(el).trim()) { hideBadge(); return; }
    badgeEl = el;
    const size = 28;
    const x = Math.min(window.innerWidth - size - 4, Math.max(4, r.right - size - 6));
    const y = r.height < 64 ? r.top + (r.height - size) / 2 : r.bottom - size - 6;
    pair.style.left = x + 'px';
    pair.style.top = Math.min(window.innerHeight - size - 4, Math.max(4, y)) + 'px';
    if (!pair.classList.contains('on')) {
      pair.classList.add('on');
      requestAnimationFrame(() => pair.classList.add('in'));
    }
  }
  function hideBadge() { badgeEl = null; if (pair) pair.classList.remove('on', 'in'); }
  // Follows the box when the page lays out again; goes away with the box or its text.
  setInterval(() => { if (badgeEl) placeBadge(badgeEl); }, 700);

  // Scanning: a turning ring. Done: the number of suggestions on a ring split by category
  // colour, or a green tick. Not checked yet: the pen-and-tick icon.
  function paintChecker() {
    if (!checker) return;
    checker.classList.remove('scan', 'clean', 'err');
    checker.style.removeProperty('--ring');
    const icon = '<span>' + SVG_CHECK + '</span>';
    // Scanning: the ring turns; the last count stays inside it while the draft is rechecked.
    if (busy) { checker.classList.add('scan'); checker.innerHTML = lastCount ? '<span>' + lastCount + '</span>' : icon; checker.title = 'Đang kiểm tra…'; return; }
    if (checkError) { checker.classList.add('err'); checker.innerHTML = '<span>!</span>'; checker.title = checkError; return; }
    if (!result) { checker.innerHTML = icon; checker.title = 'Mnemo – kiểm tra chính tả, ngữ pháp, văn phong'; return; }
    lastCount = Math.min(issues.length, 99);
    if (!issues.length) { checker.classList.add('clean'); checker.innerHTML = '<span>✓</span>'; checker.title = 'Không thấy lỗi'; return; }
    let at = 0;
    const parts = [];
    CATS.forEach((c) => {
      const n = issues.filter((i) => i.category === c).length;
      if (!n) return;
      const end = at + (n / issues.length) * 100;
      parts.push(`${CAT_COLOR[c]} ${at}% ${end}%`);
      at = end;
    });
    checker.style.setProperty('--ring', `conic-gradient(${parts.join(',')})`);
    checker.innerHTML = '<span>' + Math.min(issues.length, 99) + '</span>';
    checker.title = issues.length + ' gợi ý' + (result.score != null ? ' · điểm ' + result.score : '') + ' – bấm để mở Trợ lý viết';
  }

  // Tells Mnemo (the Writing assistant panel) what this draft looks like now.
  function report(open) {
    const el = issuesEl || lastEditable || composeTarget();
    invoke('mnemo_writing', {
      draft: {
        text: el ? editableText(el).slice(0, 20000) : '',
        busy,
        error: checkError,
        score: result ? result.score : null,
        tone: result ? result.tone || '' : '',
        toneFits: result ? result.tone_fits !== false : true,
        issues: issues.map((i) => ({ category: i.category, type: i.type, title: i.title, original: i.original, replacement: i.replacement, explanation: i.explanation })),
      },
      open: !!open,
    }).catch(() => {});
  }

  function runCheck(el, text, force, openAfter) {
    const seq = ++checkSeq;
    busy = true;
    checkError = '';
    lastChecked = text;
    paintChecker();
    report(openAfter);
    invoke('mnemo_assist_check', { text: text.slice(0, 20000), force: !!force }).then((res) => {
      if (seq !== checkSeq) return;
      busy = false;
      if (editableText(el) !== text) { paintChecker(); return; }
      result = res || { issues: [] };
      issues = Array.isArray(result.issues) ? result.issues.slice() : [];
      issuesEl = el;
      paintChecker();
      drawMarks();
      report(false);
    }).catch((e) => {
      if (seq !== checkSeq) return;
      busy = false;
      // Say why (no key, wrong model name, the model's answer…) instead of "no problems".
      checkError = String(e && e.message ? e.message : e).slice(0, 400);
      result = null;
      paintChecker();
      report(false);
      log('check: ' + e);
    });
  }

  function openChecker() {
    const el = lastEditable;
    if (!el) return;
    const text = editableText(el);
    if (!text.trim()) return;
    if (busy || (result && text === lastChecked)) { report(true); return; }
    runCheck(el, text, true, true);
  }

  function openAssist() {
    const el = lastEditable;
    if (!el) return;
    const text = editableText(el);
    if (!text.trim()) return;
    const r = caretRect(el);
    invoke('mnemo_assist_open', {
      text: text.slice(0, 20000),
      rect: { x: r.left, y: r.top, width: r.width, height: r.height },
      viewport: { width: window.innerWidth, height: window.innerHeight },
    }).catch((e) => log('assist: ' + e));
  }

  function onTyping() {
    const el = editableRoot(document.activeElement);
    if (!isEditable(el)) return;
    lastEditable = el;
    // The text changed: old suggestions no longer line up with it.
    if (!editableText(el).trim()) hideBadge();
    if (issues.length || result || checkError) { issues = []; result = null; checkError = ''; if (marks) marks.innerHTML = ''; hideCard(0); paintChecker(); report(false); }
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      if (!runtime.assist) return;
      const text = editableText(el);
      if (text.trim().length < 3 || (document.activeElement !== el && !el.contains(document.activeElement))) return;
      placeBadge(el);
      if (runtime.autoCheck && text !== lastChecked) runCheck(el, text, false, false);
      else report(false);
    }, 900);
  }
  document.addEventListener('input', onTyping, true);
  // A box that already holds a draft shows the checker as soon as it is focused.
  document.addEventListener('focusin', () => {
    const el = editableRoot(document.activeElement);
    if (runtime.assist && isEditable(el) && editableText(el).trim().length >= 3) { lastEditable = el; placeBadge(el); }
  }, true);
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.shiftKey && (e.code === 'Space' || e.key === ' ')) {
      const el = editableRoot(document.activeElement);
      if (isEditable(el)) { lastEditable = el; e.preventDefault(); e.stopPropagation(); openChecker(); }
    }
  }, true);

  // ---------- underlines, highlight and the suggestion card ----------
  function textNodes(root) {
    const out = [];
    const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let n;
    while ((n = w.nextNode())) out.push(n);
    return out;
  }
  // A Range over the first occurrence of `needle` in the box's text (across text nodes).
  function rangeOf(root, needle) {
    if (!root || !needle || root.tagName === 'TEXTAREA' || root.tagName === 'INPUT') return null;
    const nodes = textNodes(root);
    let full = '';
    const starts = [];
    nodes.forEach((n) => { starts.push(full.length); full += n.nodeValue.replace(/ /g, ' '); });
    const at = full.indexOf(needle.replace(/ /g, ' '));
    if (at < 0) return null;
    const end = at + needle.length;
    // Start: the node holding character `off`; end: the node holding the character before it.
    const pos = (off, isEnd) => {
      for (let i = 0; i < nodes.length; i++) {
        const len = nodes[i].nodeValue.length;
        if (isEnd ? off > starts[i] && off <= starts[i] + len : off >= starts[i] && off < starts[i] + len) return [nodes[i], off - starts[i]];
      }
      return null;
    };
    const a = pos(at, false), b = pos(end, true);
    if (!a || !b) return null;
    const r = document.createRange();
    try { r.setStart(a[0], a[1]); r.setEnd(b[0], b[1]); } catch (e) { return null; }
    return r;
  }
  let drawQueued = false;
  function drawMarks() {
    if (drawQueued) return;
    drawQueued = true;
    requestAnimationFrame(() => {
      drawQueued = false;
      if (!marks) return;
      const frag = document.createDocumentFragment();
      if (issuesEl && document.contains(issuesEl)) {
        const box = issuesEl.getBoundingClientRect();
        issues.forEach((it) => {
          const r = rangeOf(issuesEl, it.original);
          it._rects = r ? Array.from(r.getClientRects()).filter((c) => c.width > 0) : [];
          it._rects.forEach((c) => {
            if (c.bottom < box.top - 2 || c.top > box.bottom + 2) return;
            const u = document.createElement('div');
            u.className = 'ul';
            u.style.cssText = `left:${c.left}px;top:${c.bottom - 1}px;width:${c.width}px;background:${CAT_COLOR[it.category] || CAT_COLOR.correctness}`;
            frag.appendChild(u);
          });
        });
      }
      marks.replaceChildren(frag);
    });
  }
  function highlight(it) {
    if (!glow || !issuesEl) return;
    const r = rangeOf(issuesEl, it.original);
    if (!r) return;
    const rects = Array.from(r.getClientRects()).filter((c) => c.width > 0);
    if (rects.length) {
      const first = rects[0];
      const box = issuesEl.getBoundingClientRect();
      if (first.top < box.top || first.bottom > box.bottom) {
        const sc = issuesEl.closest('[style*="overflow"], .scrollable') || issuesEl;
        sc.scrollTop += first.top - box.top - box.height / 3;
        return setTimeout(() => highlight(it), 60);
      }
    }
    const color = CAT_COLOR[it.category] || CAT_COLOR.correctness;
    glow.replaceChildren(...rects.map((c) => {
      const d = document.createElement('div');
      d.className = 'hl';
      d.style.cssText = `left:${c.left - 2}px;top:${c.top - 1}px;width:${c.width + 4}px;height:${c.height + 2}px;background:${color}33;box-shadow:0 0 0 1.5px ${color}`;
      return d;
    }));
    clearTimeout(highlight.t);
    highlight.t = setTimeout(() => glow.replaceChildren(), 4000);
  }
  function hideCard(delay) {
    clearTimeout(cardTimer);
    cardTimer = setTimeout(() => { if (card) card.classList.remove('in'); setTimeout(() => { if (card && !card.classList.contains('in')) card.style.display = 'none'; }, 150); shownIssue = null; }, delay);
  }
  function showCard(it, rect) {
    clearTimeout(cardTimer);
    if (shownIssue === it && card.classList.contains('in')) return;
    shownIssue = it;
    const color = CAT_COLOR[it.category] || CAT_COLOR.correctness;
    card.innerHTML = `<div class="t"><span class="d"></span><b>${CAT_LABEL[it.category] || ''}</b>${it.type ? `<i>· ${esc2(it.type)}</i>` : ''}</div>
      ${it.title ? `<div class="h">${esc2(it.title)}</div>` : ''}
      <del>${esc2(it.original)}</del>
      <button class="fix" title="Bấm để nhận gợi ý">${it.replacement ? esc2(it.replacement) : '(xóa)'}</button>
      ${it.explanation ? `<div class="x">${esc2(it.explanation)}</div>` : ''}
      <div class="a"><span data-a="dismiss">Bỏ qua</span><span data-a="learn">Thêm vào từ điển</span></div>`;
    // Set through the style object: a page's content policy can block style attributes.
    card.style.setProperty('--c', color);
    card.querySelector('.fix').addEventListener('click', (ev) => { ev.preventDefault(); ev.stopPropagation(); act(it, 'accept'); });
    card.querySelectorAll('[data-a]').forEach((b) => b.addEventListener('click', (ev) => { ev.preventDefault(); ev.stopPropagation(); act(it, b.dataset.a); }));
    card.style.display = 'block';
    const w = card.offsetWidth || 340, h = card.offsetHeight || 120;
    const x = Math.min(window.innerWidth - w - 8, Math.max(8, rect.left));
    let y = rect.top - h - 8;
    if (y < 8) y = rect.bottom + 8;
    card.style.left = x + 'px';
    card.style.top = y + 'px';
    requestAnimationFrame(() => card.classList.add('in'));
  }
  function act(it, what) {
    const el = issuesEl;
    hideCard(0);
    if (!it) return;
    const keep = issues.filter((x) => x !== it && x.original !== it.original);
    if (what === 'learn') invoke('mnemo_assist_learn', { word: it.original }).catch(() => {});
    if (what === 'accept' && el && document.contains(el)) {
      const r = rangeOf(el, it.original);
      if (r) {
        el.focus();
        const sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(r);
        if (!document.execCommand('insertText', false, it.replacement || '')) {
          r.deleteContents();
          if (it.replacement) r.insertNode(document.createTextNode(it.replacement));
          el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: it.replacement }));
        }
      }
    }
    // Accepting typed into the box, which cleared the list: put the rest back.
    clearTimeout(idleTimer);
    issues = keep;
    issuesEl = el;
    if (result) result.issues = keep;
    else result = { issues: keep };
    lastChecked = el ? editableText(el) : lastChecked;
    if (el && document.contains(el)) placeBadge(el);
    paintChecker();
    drawMarks();
    report(false);
  }
  document.addEventListener('mousemove', (e) => {
    if (!issues.length || !card) return;
    if (card.classList.contains('in')) {
      const r = card.getBoundingClientRect();
      if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) return;
    }
    for (const it of issues) {
      for (const c of it._rects || []) {
        if (e.clientX >= c.left - 1 && e.clientX <= c.right + 1 && e.clientY >= c.top - 2 && e.clientY <= c.bottom + 3) { showCard(it, c); return; }
      }
    }
    if (shownIssue) hideCard(500);
  }, true);
  const redraw = () => { if (issues.length) drawMarks(); if (glow) glow.replaceChildren(); };
  window.addEventListener('scroll', redraw, true);
  window.addEventListener('resize', redraw);

  // Called by Mnemo's Writing assistant panel.
  window.__MNEMO_ISSUE__ = function (what, original) {
    if (what === 'recheck') {
      const el = issuesEl || lastEditable || composeTarget();
      if (el && document.contains(el)) { const t = editableText(el); if (t.trim()) runCheck(el, t, true, false); }
      return;
    }
    if (what === 'refresh') { refreshRuntime(); report(false); return; }
    // The panel opened: check the draft now unless that exact text was just checked.
    if (what === 'open') {
      const el = (lastEditable && document.contains(lastEditable)) ? lastEditable : composeTarget();
      if (el) lastEditable = el;
      const t = el ? editableText(el) : '';
      if (!t.trim() || busy || (result && t === lastChecked)) { report(false); return; }
      runCheck(el, t, true, false);
      return;
    }
    const it = issues.find((x) => x.original === original);
    if (!it) return;
    if (what === 'focus') highlight(it);
    else act(it, what);
  };

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
    issues = []; result = null;
    if (marks) marks.innerHTML = '';
    paintChecker();
    return true;
  };
})();
