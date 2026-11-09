// New-message notifications in the corner of the screen.
'use strict';

const timers = new Map();
let duration = 8;
let hovered = false;

function timeText(ms) {
  return new Date(ms).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
}

function avatar(n) {
  const mini = svcMini(n.service, 17);
  const inner = n.avatar
    ? `<img src="${n.avatar}" alt="">`
    : `<span class="initial" style="background:var(--accent)">${esc(Array.from(n.to || n.app || '?')[0].toUpperCase())}</span>`;
  return `<span class="avatar av" style="width:40px;height:40px">${inner}${mini}</span>`;
}

function schedule(id) {
  if (timers.has(id)) return;
  timers.set(id, setTimeout(function tick() {
    if (hovered) { timers.set(id, setTimeout(tick, 1000)); return; }
    timers.delete(id);
    invoke('notice_dismiss', { id });
  }, duration * 1000));
}

function render(list) {
  const box = $('#list');
  // Newest at the bottom, next to the taskbar; only the latest few.
  const shown = list.slice(0, 4).reverse();
  box.innerHTML = shown.map((n) => `<div class="note" data-id="${esc(n.id)}" title="Bấm để mở hội thoại">
    ${avatar(n)}
    <div class="head"><span class="app" data-noi18n>${esc(n.app)}${n.to ? ` - ${esc(n.to)}` : ''}</span><span class="time">${timeText(n.ts)}</span></div>
    <div class="from" data-noi18n>${esc(n.from)}</div>
    <div class="body" data-noi18n>${esc(n.text)}</div>
    <button class="x" data-x="${esc(n.id)}" title="Đóng">${ICON.close}</button>
  </div>`).join('');
  box.querySelectorAll('.note').forEach((el) => el.addEventListener('click', () => invoke('notice_open', { id: el.dataset.id }).catch(() => {})));
  box.querySelectorAll('[data-x]').forEach((b) => b.addEventListener('click', (e) => {
    e.stopPropagation();
    invoke('notice_dismiss', { id: b.dataset.x });
  }));
  const ids = new Set(list.map((n) => n.id));
  for (const [id, t] of timers) if (!ids.has(id)) { clearTimeout(t); timers.delete(id); }
  list.forEach((n) => schedule(n.id));
}

function apply(payload) {
  duration = payload.duration || 8;
  if (payload.icons) window.SVC_ICONS = payload.icons;
  if (payload.theme) applyTheme(payload.theme);
  setLang(payload.language);
  render(payload.notices || []);
}

document.addEventListener('mouseenter', () => { hovered = true; });
document.addEventListener('mouseleave', () => { hovered = false; });
listen('notices', (e) => apply(e.payload));
invoke('notices_state').then(apply).catch(() => {});
