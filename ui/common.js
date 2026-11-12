// Helpers shared by the shell (app.js) and the writing popup (assist.js).
'use strict';

const TAURI = window.__TAURI__;
const invoke = (cmd, args) => TAURI.core.invoke(cmd, args || {});
const listen = (ev, fn) => TAURI.event.listen(ev, fn);

const $ = (sel, root) => (root || document).querySelector(sel);
const $$ = (sel, root) => Array.from((root || document).querySelectorAll(sel));

function esc(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function el(html) {
  const t = document.createElement('template');
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

function errText(e) {
  if (!e) return 'Lỗi không rõ';
  if (typeof e === 'string') return e;
  return e.message || JSON.stringify(e);
}

function toast(msg, isError) {
  const host = document.getElementById('toasts');
  if (!host) return;
  const t = el(`<div class="toast${isError ? ' error' : ''}">${esc(msg)}</div>`);
  host.appendChild(t);
  setTimeout(() => t.remove(), isError ? 7000 : 3500);
}

function applyTheme(theme) {
  const root = document.documentElement;
  if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
  else delete root.dataset.theme;
}

// Minimal, safe Markdown: paragraphs, bullet lists, **bold**, `code`. Everything is escaped first.
function md(text) {
  const lines = esc(text || '').split(/\r?\n/);
  let html = '';
  let inList = false;
  const inline = (s) => s.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
  for (const raw of lines) {
    const line = raw.trimEnd();
    const m = /^\s*(?:[-*•]|\d+[.)])\s+(.*)$/.exec(line);
    if (m) {
      if (!inList) { html += '<ul>'; inList = true; }
      html += `<li>${inline(m[1])}</li>`;
      continue;
    }
    if (inList) { html += '</ul>'; inList = false; }
    if (!line.trim()) continue;
    const h = /^#{1,4}\s+(.*)$/.exec(line);
    html += h ? `<p><b>${inline(h[1])}</b></p>` : `<p>${inline(line)}</p>`;
  }
  if (inList) html += '</ul>';
  return `<div class="md" data-noi18n>${html}</div>`;
}

const ICON = {
  archive: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="5" rx="1.5"/><path d="M5 9v9a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9M10 13h4"/></svg>',
  media: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 16-5-5-9 9"/></svg>',
  import: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12m0 0-4-4m4 4 4-4"/><path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"/></svg>',
  settings: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  home: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V20a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1V9.5"/></svg>',
  plus: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>',
  pencheck: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 4.5l3 3L8 17H5v-3z"/><path d="M12.5 6.5l3 3"/><path d="m13 18.5 2.2 2.2 4.3-4.7"/></svg>',
  refresh: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 5v6h-6"/></svg>',
  sparkle: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"/></svg>',
  search: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>',
  close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6 6 18"/></svg>',
  file: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/></svg>',
  folder: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>',
  lock: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></svg>',
};

const SERVICE_SHORT = { whatsapp: 'WA', messenger: 'Ms', instagram: 'IG', telegram: 'Tg', zalo: 'Za', discord: 'Dc', wechat: 'Wc', threads: 'Th', slack: 'Sl', linkedin: 'in' };
const SERVICE_NAME = { whatsapp: 'WhatsApp', messenger: 'Messenger', instagram: 'Instagram', telegram: 'Telegram', zalo: 'Zalo', discord: 'Discord', wechat: 'WeChat', threads: 'Threads', slack: 'Slack', linkedin: 'LinkedIn' };

// Each service's own site icon, fetched by the app at runtime (data: URLs). Until it
// arrives (or offline), a lettered tile stands in.
window.SVC_ICONS = window.SVC_ICONS || {};

function svcIcon(service, size) {
  const s = size || 30;
  const src = window.SVC_ICONS[service];
  if (src) {
    return `<span class="svc img" style="width:${s}px;height:${s}px;padding:${Math.max(2, Math.round(s * 0.14))}px;border-radius:${Math.round(s * 0.28)}px"><img src="${src}" alt="${esc(SERVICE_NAME[service] || '')}"></span>`;
  }
  return `<span class="svc ${esc(service)}" style="width:${s}px;height:${s}px;font-size:${Math.round(s * 0.36)}px;border-radius:${Math.round(s * 0.3)}px">${SERVICE_SHORT[service] || '?'}</span>`;
}

// The small app badge in the corner of an avatar.
function svcMini(service, px) {
  const src = window.SVC_ICONS[service];
  if (src) return `<span class="mini svc img" style="width:${px}px;height:${px}px;padding:${Math.max(1, Math.round(px * 0.08))}px"><img src="${src}" alt=""></span>`;
  return `<span class="mini svc ${esc(service)}" style="width:${px}px;height:${px}px;font-size:${Math.round(px * 0.46)}px">${SERVICE_SHORT[service] || ''}</span>`;
}

function fmtTime(ms) {
  if (!ms) return '';
  const d = new Date(ms);
  return d.toLocaleString(locale(), { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });
}
function fmtDay(ms) {
  return new Date(ms).toLocaleDateString(locale(), { weekday: 'long', day: '2-digit', month: '2-digit', year: 'numeric' });
}
function fmtSize(n) {
  if (!n) return '0 B';
  const u = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
}

// Provider / model / effort / thinking picker, used by the AI panel and the popup.
function choiceBar(container, choice, providers, onChange) {
  const usable = providers.filter((p) => p.enabled);
  const p = usable.find((x) => x.id === choice.provider) || usable[0];
  if (p && p.id !== choice.provider) choice.provider = p.id;
  const efforts = p && p.kind === 'claude_code' ? ['', 'low', 'medium', 'high', 'xhigh', 'max'] : ['', 'low', 'medium', 'high'];
  const listId = 'models-' + Math.random().toString(36).slice(2);
  container.innerHTML = `
    <div class="choice">
      <select data-k="provider" class="full" title="Provider">
        ${usable.map((x) => `<option value="${esc(x.id)}" ${x.id === choice.provider ? 'selected' : ''}>${esc(x.name)}${x.kind !== 'claude_code' && x.kind !== 'codex_cli' && x.kind !== 'compatible' && !x.has_key ? ' (chưa có key)' : ''}</option>`).join('')}
      </select>
      <input data-k="model" list="${listId}" placeholder="Model (mặc định)" value="${esc(choice.model || '')}" title="Model">
      <datalist id="${listId}">${(p ? p.models : []).map((m) => `<option value="${esc(m)}">`).join('')}</datalist>
      <select data-k="effort" title="Effort">
        ${efforts.map((e) => `<option value="${e}" ${e === (choice.effort || '') ? 'selected' : ''}>Effort: ${e || (window.LANG === 'en' ? 'default' : 'mặc định')}</option>`).join('')}
      </select>
      <label class="check full small" title="Bật suy nghĩ sâu (thinking) – chậm hơn, kỹ hơn">
        <span class="switch"><input type="checkbox" data-k="thinking" ${choice.thinking ? 'checked' : ''}><span></span></span>
        Thinking
      </label>
    </div>`;
  container.querySelectorAll('[data-k]').forEach((input) => {
    input.addEventListener('change', () => {
      const k = input.dataset.k;
      choice[k] = input.type === 'checkbox' ? input.checked : input.value.trim();
      if (k === 'provider') {
        choice.model = '';
        choiceBar(container, choice, providers, onChange);
      }
      onChange(choice);
    });
  });
}
