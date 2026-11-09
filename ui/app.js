// Mnemo shell: sidebar, archive, media, import, settings, account pages and the AI panel.
'use strict';

let S = null;               // state from get_state
let page = 'home';          // home | archive | media | import | settings | account:<id>
let visible = null;         // account id shown in the content area
let panelOpen = false;
const archive = { service: '', q: '', chats: [], chatId: null, chat: null, messages: [], hits: null, search: '', around: null };
const importState = { running: false, progress: null, report: null, error: null };

async function refresh() {
  S = await invoke('get_state');
  visible = S.visible;
  applyTheme(S.settings.theme);
  window.SVC_ICONS = S.icons || {};
  setLang(S.settings.language);
  renderSidebar();
}

function accounts() { return (S && S.settings.accounts) || []; }
function accountById(id) { return accounts().find((a) => a.id === id); }

// ---------------------------------------------------------------- sidebar

function avatarHtml(a, size) {
  const src = S && S.avatars && S.avatars[a.id];
  const name = (a.label || SERVICE_NAME[a.service] || '?').trim();
  let hue = 0;
  for (const ch of name) hue = (hue * 31 + ch.codePointAt(0)) % 360;
  const mini = Math.max(14, Math.round(size * 0.42));
  return `<span class="avatar" style="width:${size}px;height:${size}px">
    ${src ? `<img src="${src}" alt="">` : `<span class="initial" style="background:hsl(${hue} 55% 52%);font-size:${Math.round(size * 0.42)}px">${esc(Array.from(name)[0].toUpperCase())}</span>`}
    ${svcMini(a.service, mini)}
  </span>`;
}

function sidebarWidth() { return S && S.settings.sidebar_compact ? 76 : 248; }

async function syncLayout() {
  document.documentElement.style.setProperty('--sidebar', sidebarWidth() + 'px');
  $('#app').classList.toggle('compact', !!(S && S.settings.sidebar_compact));
  const right = !!(S && S.settings.sidebar_right);
  $('#app').classList.toggle('right', right);
  try { await invoke('set_layout', { sidebar: sidebarWidth(), panel: panelOpen, right }); } catch (e) {}
}

function renderSidebar() {
  const sb = $('#sidebar');
  const compact = !!S.settings.sidebar_compact;
  const nav = (id, icon, label, extra) =>
    `<button class="side-item ${extra || ''} ${!visible && page === id ? 'active' : ''}" data-page="${id}" title="${label}">${ICON[icon]}<span class="side-label">${label}</span></button>`;
  const total = accounts().reduce((n, a) => n + ((S.unread || {})[a.id] || 0), 0);
  sb.innerHTML = `
    <div class="side-head">
      <img class="logo" src="logo.png" alt="Mnemo" data-page="home" title="Mnemo – Every chat in sight, locked safe and tight.">
      <div class="side-label brand grow" data-page="home"><b>Mnemo</b>${total ? `<span class="faint small">${total} chưa đọc</span>` : ''}</div>
      <button class="side-toggle" id="collapse" title="${compact ? 'Mở rộng thanh bên' : 'Thu gọn thanh bên'}">${compact !== !!S.settings.sidebar_right ? '›' : '‹'}</button>
    </div>
    <div class="side-section side-label">Tài khoản</div>
    <div class="accounts scroll">
      ${accounts().map((a) => {
        const n = (S.unread || {})[a.id] || 0;
        return `<button class="acc-row ${visible === a.id ? 'active' : ''}" data-acc="${esc(a.id)}" title="${esc(SERVICE_NAME[a.service])} · ${esc(a.label)}${n ? ` · ${n} chưa đọc` : ''} (chuột phải: cài đặt)">
          <span class="acc-avatar">${avatarHtml(a, 38)}${n && compact ? `<span class="count">${n > 99 ? '99+' : n}</span>` : ''}</span>
          <span class="acc-text side-label"><span class="acc-app">${esc(SERVICE_NAME[a.service])}</span><span class="acc-name" data-noi18n>${esc(a.label)}</span></span>
          ${n && !compact ? `<span class="pill">${n > 999 ? '999+' : n}</span>` : ''}
        </button>`;
      }).join('')}
      ${accounts().length ? '' : `<button class="acc-row add" data-page="home" title="Thêm tài khoản"><span class="acc-avatar"><span class="add-circle">${ICON.plus}</span></span><span class="acc-text side-label"><span class="acc-name">Thêm tài khoản</span></span></button>`}
    </div>
    <div class="side-nav">
      ${nav('home', 'home', 'Trang chủ')}
      ${`<button class="side-item ${panelOpen ? 'active' : ''}" data-panel title="Trợ lý AI">${ICON.sparkle}<span class="side-label">Trợ lý AI</span></button>`}
      ${nav('archive', 'archive', 'Kho tin nhắn')}
      ${nav('media', 'media', 'Tệp đã lưu')}
      ${nav('import', 'import', 'Nhập tệp xuất')}
      ${nav('settings', 'settings', 'Cài đặt')}
    </div>`;
  sb.querySelectorAll('[data-page]').forEach((b) => b.addEventListener('click', () => openPage(b.dataset.page)));
  sb.querySelectorAll('[data-acc]').forEach((b) => {
    b.addEventListener('click', () => showAccount(b.dataset.acc));
    b.addEventListener('contextmenu', (e) => { e.preventDefault(); openPage('account:' + b.dataset.acc); });
  });
  sb.querySelector('[data-panel]').addEventListener('click', () => togglePanel());
  $('#collapse').addEventListener('click', async () => {
    S.settings.sidebar_compact = !S.settings.sidebar_compact;
    await syncLayout();
    renderSidebar();
    saveSettingsSoon();
  });
}

async function showAccount(id) {
  try {
    await invoke('show_account', { id });
    visible = id;
    renderSidebar();
    if (panelOpen) renderPanel();
  } catch (e) { toast(errText(e), true); }
}

async function openPage(p, scrollToAdd) {
  await invoke('show_page');
  visible = null;
  page = p;
  renderSidebar();
  await renderMain();
  if (panelOpen) renderPanel();
  if (scrollToAdd) { const t = $('#add-accounts'); if (t) t.scrollIntoView({ behavior: 'smooth' }); }
}

async function renderMain() {
  const main = $('#main');
  try {
    if (page === 'home') return renderHome(main);
    if (page === 'archive') return renderArchive(main);
    if (page === 'media') return renderMedia(main);
    if (page === 'import') return renderImport(main);
    if (page === 'settings') return renderSettings(main);
    if (page.startsWith('account:')) return renderAccount(main, page.slice(8));
  } catch (e) {
    main.innerHTML = `<div class="page"><div class="notice danger">${esc(errText(e))}</div></div>`;
  }
}

// ---------------------------------------------------------------- home

function renderHome(main) {
  const st = S.archive.ok ? S.archive.stats : null;
  const accs = accounts();
  main.innerHTML = `<div class="page">
    <div class="card hero">
      <img src="logo.png" alt="">
      <div class="grow">
        <h1>Mnemo</h1>
        <div class="slogan">Every chat in sight, locked safe and tight.</div>
      </div>
      ${st ? `<div class="row">
        <div class="stat"><span class="faint small">Hội thoại</span><b>${st.chats}</b></div>
        <div class="stat"><span class="faint small">Tin nhắn</span><b>${st.messages}</b></div>
        <div class="stat"><span class="faint small">Tệp</span><b>${st.media}</b></div>
      </div>` : ''}
    </div>
    ${S.archive.ok ? '' : `<div class="notice danger section">Kho lưu trữ chưa mở: ${esc(S.archive.error)}<br><a href="#" data-goto="settings">Mở Cài đặt để nhập khóa khôi phục hoặc chọn thư mục khác.</a></div>`}
    ${accs.length ? `<div class="section"><h2>Tài khoản</h2><div class="grid">
      ${accs.map((a) => {
        const stats = (S.stats || {})[a.id] || {};
        const n = (S.unread || {})[a.id] || 0;
        return `<div class="card svc-card">
          <div class="row">${avatarHtml(a, 42)}<div class="grow"><div class="small muted">${SERVICE_NAME[a.service]}</div><b class="ellipsis" data-noi18n>${esc(a.label)}</b></div>
          ${n ? `<span class="badge danger">${n} chưa đọc</span>` : ''}</div>
          <div class="small muted">${a.capture && S.settings.capture_enabled ? `Đang lưu tin nhắn · phiên này: ${stats.messages || 0} tin, ${stats.media || 0} tệp` : 'Không lưu tin nhắn'}</div>
          <div class="row"><button class="btn sm primary" data-open="${esc(a.id)}">Mở</button><button class="btn sm" data-acc-page="${esc(a.id)}">Cài đặt</button></div>
        </div>`;
      }).join('')}</div></div>` : ''}
    <div class="section" id="add-accounts"><h2>Thêm tài khoản</h2>
      <p class="muted small">Mỗi tài khoản có hồ sơ trình duyệt riêng, nên bạn thêm được nhiều tài khoản của cùng một ứng dụng.</p>
      <div class="grid">
        ${S.services.map((s) => `<div class="card svc-card">
          <div class="row">${svcIcon(s.id, 40)}<b>${esc(s.name)}</b></div>
          <div class="small muted ellipsis">${esc(s.url)}</div>
          <button class="btn sm" data-add-svc="${s.id}">${ICON.plus} Thêm ${esc(s.name)}</button>
        </div>`).join('')}
      </div>
    </div>
  </div>`;
  main.querySelectorAll('[data-open]').forEach((b) => b.addEventListener('click', () => showAccount(b.dataset.open)));
  main.querySelectorAll('[data-acc-page]').forEach((b) => b.addEventListener('click', () => openPage('account:' + b.dataset.accPage)));
  main.querySelectorAll('[data-goto]').forEach((b) => b.addEventListener('click', (e) => { e.preventDefault(); openPage(b.dataset.goto); }));
  main.querySelectorAll('[data-add-svc]').forEach((b) => b.addEventListener('click', () => addAccount(b.dataset.addSvc)));
}

function addAccount(service) {
  const back = el(`<div class="modal-back"><div class="card modal">
    <div class="row">${svcIcon(service, 36)}<h2>Thêm ${SERVICE_NAME[service]}</h2></div>
    <label class="col"><span class="muted small">Tên hiển thị (để phân biệt nhiều tài khoản)</span>
      <input type="text" id="new-label" placeholder="${SERVICE_NAME[service]}" maxlength="60"></label>
    <div class="row" style="justify-content:flex-end"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-ok>Thêm và đăng nhập</button></div>
  </div></div>`);
  document.body.appendChild(back);
  const input = back.querySelector('#new-label');
  input.focus();
  const close = () => back.remove();
  back.querySelector('[data-cancel]').addEventListener('click', close);
  back.addEventListener('click', (e) => { if (e.target === back) close(); });
  let adding = false;
  const ok = async () => {
    if (adding) return;
    adding = true;
    const btn = back.querySelector('[data-ok]');
    btn.disabled = true;
    btn.insertAdjacentHTML('afterbegin', '<span class="spinner"></span>');
    try {
      const acc = await invoke('add_account', { service, label: input.value });
      close();
      await refresh();
      visible = acc.id;
      renderSidebar();
    } catch (e) { adding = false; btn.disabled = false; const sp = btn.querySelector('.spinner'); if (sp) sp.remove(); toast(errText(e), true); }
  };
  back.querySelector('[data-ok]').addEventListener('click', ok);
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') ok(); if (e.key === 'Escape') close(); });
}

// ---------------------------------------------------------------- archive

async function loadChats() {
  archive.chats = await invoke('list_chats', { service: archive.service || null, account: null, query: archive.q || null });
}

async function renderArchive(main) {
  if (!S.archive.ok) {
    main.innerHTML = `<div class="page"><div class="notice danger">Kho lưu trữ chưa mở: ${esc(S.archive.error)}</div></div>`;
    return;
  }
  await loadChats();
  main.innerHTML = `<div class="archive">
    <div class="left">
      <div class="filters">
        <div class="row"><input type="search" id="fts" class="grow" placeholder="Tìm trong mọi tin nhắn (không cần dấu)" value="${esc(archive.search)}"></div>
        <div class="row">
          <select id="svc-filter" class="grow"><option value="">Tất cả ứng dụng</option>
            ${S.services.map((s) => `<option value="${s.id}" ${archive.service === s.id ? 'selected' : ''}>${esc(s.name)}</option>`).join('')}
          </select>
          <input type="search" id="chat-filter" class="grow" placeholder="Lọc hội thoại" value="${esc(archive.q)}">
        </div>
      </div>
      <div class="chat-list scroll" id="chat-list"></div>
    </div>
    <div class="right" id="conv"></div>
  </div>`;
  $('#svc-filter').addEventListener('change', async (e) => { archive.service = e.target.value; await loadChats(); renderChatList(); });
  let t;
  $('#chat-filter').addEventListener('input', (e) => { clearTimeout(t); t = setTimeout(async () => { archive.q = e.target.value; await loadChats(); renderChatList(); }, 250); });
  $('#fts').addEventListener('keydown', async (e) => {
    if (e.key !== 'Enter') return;
    archive.search = e.target.value.trim();
    if (!archive.search) { archive.hits = null; renderChatList(); return; }
    try {
      archive.hits = await invoke('search_messages', { query: archive.search, service: archive.service || null, offset: 0 });
      renderChatList();
    } catch (err) { toast(errText(err), true); }
  });
  renderChatList();
  renderConversation();
}

function renderChatList() {
  const list = $('#chat-list');
  if (!list) return;
  if (archive.hits) {
    list.innerHTML = `<div class="row" style="padding:10px 14px"><button class="btn sm" id="back-chats">← Danh sách hội thoại</button><span class="small muted">${archive.hits.length} kết quả</span></div>
      <div class="hits">${archive.hits.map((h, i) => `<div class="hit-item" data-i="${i}">
        <div class="row small">${svcIcon(h.service, 18)}<b class="ellipsis grow">${esc(h.chat_title || 'Hội thoại')}</b><span class="faint">${fmtTime(h.ts)}</span></div>
        <div class="small" data-noi18n><span class="muted">${esc(h.sender || '')}${h.sender ? ': ' : ''}</span>${esc(h.snippet).replace(/\[\[/g, '<mark>').replace(/\]\]/g, '</mark>')}</div>
      </div>`).join('') || '<div class="empty">Không tìm thấy</div>'}</div>`;
    $('#back-chats').addEventListener('click', () => { archive.hits = null; renderChatList(); });
    list.querySelectorAll('.hit-item').forEach((n) => n.addEventListener('click', () => {
      const h = archive.hits[+n.dataset.i];
      openChat(h.chat_id, h.message_id);
    }));
    return;
  }
  if (!archive.chats.length) {
    list.innerHTML = `<div class="empty"><div>${ICON.archive}</div><div>Chưa có hội thoại nào được lưu.</div><div class="small">Mở một ứng dụng và vào hội thoại, hoặc nhập tệp xuất.</div></div>`;
    return;
  }
  list.innerHTML = archive.chats.map((c) => {
    const acc = accountById(c.account);
    return `<div class="chat-item ${archive.chatId === c.id ? 'active' : ''}" data-id="${c.id}">
      ${svcIcon(c.service, 30)}
      <div class="grow">
        <div class="row"><b class="ellipsis grow" data-noi18n>${esc(c.title || c.chat_key)}</b>${c.unseen ? `<span class="badge accent">${c.unseen}</span>` : ''}</div>
        <div class="meta"><span class="ellipsis">${esc(acc ? acc.label : SERVICE_NAME[c.service])}${c.is_group ? ' · nhóm' : ''}</span><span>${c.count} tin</span></div>
      </div>
    </div>`;
  }).join('');
  list.querySelectorAll('.chat-item').forEach((n) => n.addEventListener('click', () => openChat(+n.dataset.id)));
}

async function openChat(chatId, aroundId) {
  archive.chatId = chatId;
  archive.chat = archive.chats.find((c) => c.id === chatId) || (await invoke('list_chats', {})).find((c) => c.id === chatId) || null;
  archive.around = aroundId || null;
  archive.messages = await invoke('chat_messages', { chatId, beforeOrd: null, beforeId: null, around: aroundId || null });
  renderChatList();
  renderConversation();
  if (panelOpen) renderPanel();
}

function messageHtml(m, chat) {
  if (m.kind === 'service') return `<div class="msg service" data-noi18n>${esc(m.text)}</div>`;
  const out = m.outgoing === true;
  const who = !out && chat && chat.is_group && m.sender ? `<div class="who" data-noi18n>${esc(m.sender)}</div>` : '';
  const when = m.ts_exact ? fmtTime(m.ts) : `${esc(m.time_text || '')} ≈ ${fmtTime(m.ts)}`;
  const media = m.media.map((x) => x.kind === 'image' && x.size < 8 * 1024 * 1024
    ? `<img class="preview" data-media="${x.id}" alt="${esc(x.file_name)}" title="${esc(x.file_name)}">`
    : `<span class="file-chip" data-open-media="${x.id}">${ICON.file}<span data-noi18n>${esc(x.file_name)}</span><span class="faint">${fmtSize(x.size)}</span>${x.encrypted ? ICON.lock : ''}</span>`).join('');
  const text = m.deleted ? '<span class="deleted">Tin nhắn đã bị thu hồi</span>' : `<span data-noi18n>${esc(m.text)}</span>`;
  return `<div class="msg ${out ? 'out' : ''} ${archive.around === m.id ? 'hit' : ''}" data-mid="${m.id}">${who}${text}${media}<div class="when">${when}${m.edited ? ' · đã sửa' : ''}</div></div>`;
}

function renderConversation() {
  const conv = $('#conv');
  if (!conv) return;
  const chat = archive.chat;
  if (!chat) {
    conv.innerHTML = `<div class="empty"><div>Chọn một hội thoại để xem tin nhắn đã lưu.</div></div>`;
    return;
  }
  const acc = accountById(chat.account);
  let lastDay = '';
  const rows = archive.messages.map((m) => {
    const day = new Date(m.ts).toDateString();
    const sep = day !== lastDay ? `<div class="day-sep">${fmtDay(m.ts)}</div>` : '';
    lastDay = day;
    return sep + messageHtml(m, chat);
  }).join('');
  conv.innerHTML = `
    <div class="conv-head">${svcIcon(chat.service, 34)}
      <div class="grow"><b class="ellipsis" data-noi18n>${esc(chat.title || chat.chat_key)}</b>
        <div class="small muted">${esc(acc ? acc.label : SERVICE_NAME[chat.service])} · ${chat.count} tin${chat.unseen ? ` · ${chat.unseen} chưa xem` : ''}</div></div>
      ${chat.unseen ? '<button class="btn sm" id="seen">Đánh dấu đã xem</button>' : ''}
      <button class="btn sm" id="ai-chat">${ICON.sparkle} AI</button>
    </div>
    <div class="messages scroll" id="msgs">
      ${archive.messages.length >= 60 ? '<button class="btn sm" id="older" style="align-self:center">Tải tin cũ hơn</button>' : ''}
      ${rows || '<div class="empty">Chưa có tin nhắn</div>'}
    </div>`;
  const box = $('#msgs');
  const hit = box.querySelector('.msg.hit');
  if (hit) hit.scrollIntoView({ block: 'center' }); else box.scrollTop = box.scrollHeight;
  wireMedia(box);
  const seen = $('#seen');
  if (seen) seen.addEventListener('click', async () => { await invoke('mark_seen', { chatId: chat.id }); await loadChats(); archive.chat = archive.chats.find((c) => c.id === chat.id) || chat; renderChatList(); renderConversation(); });
  $('#ai-chat').addEventListener('click', () => togglePanel(true));
  const older = $('#older');
  if (older) older.addEventListener('click', async () => {
    const first = archive.messages[0];
    const more = await invoke('chat_messages', { chatId: chat.id, beforeOrd: first.ord, beforeId: first.id, around: null });
    if (!more.length) { older.remove(); return; }
    const prevHeight = box.scrollHeight;
    archive.messages = more.concat(archive.messages);
    archive.around = null;
    renderConversation();
    const b = $('#msgs');
    b.scrollTop = b.scrollHeight - prevHeight;
  });
}

function wireMedia(root) {
  const io = new IntersectionObserver((entries) => {
    entries.forEach(async (en) => {
      if (!en.isIntersecting) return;
      io.unobserve(en.target);
      try {
        const url = await invoke('media_preview', { id: +en.target.dataset.media });
        if (url) en.target.src = url; else en.replaceWith(el(`<span class="file-chip">${ICON.file}${esc(en.target.alt)}</span>`));
      } catch (e) { en.target.alt = 'Không xem trước được'; }
    });
  });
  root.querySelectorAll('img[data-media]').forEach((img) => {
    io.observe(img);
    img.addEventListener('click', () => invoke('open_media', { id: +img.dataset.media }).catch((e) => toast(errText(e), true)));
  });
  root.querySelectorAll('[data-open-media]').forEach((n) => n.addEventListener('click', () => invoke('open_media', { id: +n.dataset.openMedia }).catch((e) => toast(errText(e), true))));
}

// ---------------------------------------------------------------- media

async function renderMedia(main) {
  if (!S.archive.ok) { main.innerHTML = `<div class="page"><div class="notice danger">${esc(S.archive.error)}</div></div>`; return; }
  const items = await invoke('list_media', { offset: 0 });
  const origin = { live: 'Tự lưu', import: 'Nhập', download: 'Tải về' };
  main.innerHTML = `<div class="page">
    <div class="page-head"><div class="grow"><h1>Tệp đã lưu</h1><p class="muted">Lưu tại <span class="mono">${esc(S.settings.downloads_dir)}</span></p></div>
      <button class="btn" data-folder="downloads">${ICON.folder} Mở thư mục</button></div>
    ${items.length ? `<div class="card"><table class="table"><thead><tr><th></th><th>Tên</th><th>Loại</th><th>Dung lượng</th><th>Nguồn</th><th>Lúc lưu</th><th></th></tr></thead><tbody>
      ${items.map((m) => `<tr>
        <td>${m.kind === 'image' ? `<img class="thumb" data-media="${m.id}" alt="${esc(m.file_name)}">` : `<span class="thumb row" style="justify-content:center">${ICON.file}</span>`}</td>
        <td><div class="ellipsis" style="max-width:340px" title="${esc(m.path)}" data-noi18n>${esc(m.file_name)}</div>${m.encrypted ? '<span class="badge accent">Mã hóa</span>' : ''}</td>
        <td>${esc(m.kind)}</td><td>${fmtSize(m.size)}</td><td><span class="badge">${origin[m.origin] || esc(m.origin)}</span></td>
        <td class="small muted">${fmtTime(m.saved_at)}</td>
        <td><div class="row"><button class="btn sm" data-open-media="${m.id}">Mở</button><button class="btn sm ghost" data-reveal="${m.id}">${ICON.folder}</button></div></td>
      </tr>`).join('')}</tbody></table></div>` : '<div class="card pad empty">Chưa có tệp nào. Ảnh và tệp trong các hội thoại sẽ được lưu theo quy tắc trong Cài đặt.</div>'}
  </div>`;
  wireMedia(main);
  main.querySelectorAll('[data-reveal]').forEach((b) => b.addEventListener('click', () => invoke('reveal_media', { id: +b.dataset.reveal }).catch((e) => toast(errText(e), true))));
  main.querySelectorAll('[data-folder]').forEach((b) => b.addEventListener('click', () => invoke('open_folder', { which: b.dataset.folder }).catch((e) => toast(errText(e), true))));
}

// ---------------------------------------------------------------- import

const IMPORTS = [
  {
    kind: 'telegram', services: ['telegram'], title: 'Telegram (JSON)',
    how: 'Trên máy tính, mở <b>Telegram Desktop</b> → Settings → Advanced → <b>Export Telegram data</b> (hoặc mở một chat → ⋮ → Export chat history). Chọn định dạng <b>Machine-readable JSON</b> và tích Photos/Files nếu muốn lấy cả tệp. Lần xuất đầu tiên Telegram có thể bắt chờ 24 giờ.',
    pick: [{ label: 'Chọn thư mục xuất', folder: true }, { label: 'Chọn result.json', ext: ['json'] }],
  },
  {
    kind: 'whatsapp', services: ['whatsapp'], title: 'WhatsApp (Export chat)',
    how: 'Trên điện thoại, mở hội thoại → ⋮ (hoặc tên hội thoại) → <b>Thêm → Xuất đoạn chat</b> → chọn <b>Kèm tệp phương tiện</b> nếu muốn có ảnh, rồi gửi tệp <b>.zip</b> về máy tính. Mỗi lần xuất là một hội thoại.',
    pick: [{ label: 'Chọn tệp .zip hoặc .txt', ext: ['zip', 'txt'] }],
  },
  {
    kind: 'meta', services: ['messenger', 'instagram'], title: 'Messenger / Instagram (Tải thông tin của bạn)',
    how: 'Vào <b>Trung tâm tài khoản</b> → Thông tin và quyền của bạn → <b>Tải thông tin của bạn</b>. Chọn tài khoản Facebook hoặc Instagram, chọn mục <b>Tin nhắn</b>, định dạng <b>JSON</b>. Khi Meta gửi xong, tải tệp .zip về (có thể nhiều phần, nhập lần lượt từng tệp).',
    pick: [{ label: 'Chọn tệp .zip', ext: ['zip'] }, { label: 'Chọn thư mục đã giải nén', folder: true }],
  },
];

function renderImport(main) {
  const accs = accounts();
  main.innerHTML = `<div class="page">
    <div class="page-head"><div class="grow"><h1>Nhập từ tệp xuất</h1><p class="muted">Lấp lịch sử cũ từ bản xuất chính thức của ứng dụng. Nhập lại nhiều lần không bị trùng.</p></div></div>
    ${importState.running ? `<div class="card pad section col"><b>Đang nhập…</b><div class="progress"><div></div></div><div class="small muted" id="imp-progress">${importState.progress ? `${importState.progress.chats} hội thoại · ${importState.progress.messages} tin · ${importState.progress.media} tệp — ${esc(importState.progress.current)}` : 'Đang đọc tệp…'}</div></div>` : ''}
    ${importState.report ? `<div class="notice ok section">Xong: ${importState.report.chats} hội thoại, ${importState.report.messages_new} tin mới, ${importState.report.messages_updated} tin cập nhật, ${importState.report.media_saved} tệp lưu, ${importState.report.media_skipped} tệp bỏ qua theo quy tắc.
      ${importState.report.errors.length ? `<div class="small" style="margin-top:6px">${importState.report.errors.map(esc).join('<br>')}</div>` : ''}</div>` : ''}
    ${importState.error ? `<div class="notice danger section">${esc(importState.error)}</div>` : ''}
    ${IMPORTS.map((imp) => {
      const targets = accs.filter((a) => imp.services.includes(a.service));
      return `<div class="card pad section col" data-kind="${imp.kind}">
        <h2>${esc(imp.title)}</h2>
        <div class="small muted">${imp.how}</div>
        ${targets.length ? `<div class="row wrap">
          <span class="small muted">Nhập vào</span>
          <select data-target>${targets.map((a) => `<option value="${esc(a.id)}">${esc(a.label)} (${SERVICE_NAME[a.service]})</option>`).join('')}</select>
          ${imp.pick.map((p, i) => `<button class="btn" data-pick="${i}" ${importState.running ? 'disabled' : ''}>${p.folder ? ICON.folder : ICON.file} ${esc(p.label)}</button>`).join('')}
        </div>` : `<div class="notice">Hãy thêm tài khoản ${imp.services.map((s) => SERVICE_NAME[s]).join(' hoặc ')} trước.</div>`}
      </div>`;
    }).join('')}
    <p class="small faint">Zalo, Discord, WeChat, Threads, Slack và LinkedIn chưa có bản xuất chính thức mà Mnemo đọc được; tin nhắn của chúng được lưu khi bạn mở hội thoại trong Mnemo.</p>
  </div>`;
  main.querySelectorAll('[data-kind]').forEach((card) => {
    const imp = IMPORTS.find((x) => x.kind === card.dataset.kind);
    card.querySelectorAll('[data-pick]').forEach((b) => b.addEventListener('click', async () => {
      const p = imp.pick[+b.dataset.pick];
      const path = p.folder ? await invoke('pick_folder', { title: p.label }) : await invoke('pick_file', { title: p.label, extensions: p.ext });
      if (!path) return;
      const accountId = card.querySelector('[data-target]').value;
      Object.assign(importState, { running: true, progress: null, report: null, error: null });
      renderImport(main);
      try {
        importState.report = await invoke('run_import', { kind: imp.kind, path, accountId });
      } catch (e) {
        importState.error = errText(e);
      }
      importState.running = false;
      await refresh();
      if (page === 'import') renderImport($('#main'));
    }));
  });
}

// ---------------------------------------------------------------- settings

let saveTimer = null;
function saveSettingsSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try { await invoke('save_settings', { settings: S.settings }); } catch (e) { toast(errText(e), true); }
  }, 350);
}

function rulesEditor(rules, onChange) {
  const box = el('<div class="form"></div>');
  const cats = [['images', 'Ảnh'], ['videos', 'Video'], ['audio', 'Âm thanh / tin nhắn thoại'], ['documents', 'Tài liệu (pdf, docx, xlsx, zip…)'], ['other', 'Định dạng khác']];
  box.innerHTML = `
    <span class="label">Tự động lưu tệp</span><label class="check"><span class="switch"><input type="checkbox" data-r="enabled" ${rules.enabled ? 'checked' : ''}><span></span></span>Bật</label>
    <span class="label">Loại tệp</span><div class="row wrap">${cats.map(([k, l]) => `<label class="check"><input type="checkbox" data-r="${k}" ${rules[k] ? 'checked' : ''}>${l}</label>`).join('')}</div>
    <span class="label">Dung lượng tối đa mỗi tệp (MB)</span><input type="number" min="0" max="512" data-r="max_size_mb" value="${rules.max_size_mb}" style="width:120px">
    <span class="hint">0 = không giới hạn (tối đa vẫn là 512 MB).</span>
    <span class="label">Lưu cả trong nhóm</span><label class="check"><span class="switch"><input type="checkbox" data-r="include_groups" ${rules.include_groups ? 'checked' : ''}><span></span></span>Có</label>
    <span class="label">Luôn cho phép đuôi tệp</span><input type="text" data-r="allow_extensions" data-list value="${esc(rules.allow_extensions.join(', '))}" placeholder="vd: dwg, psd">
    <span class="label">Không bao giờ lưu đuôi tệp</span><input type="text" data-r="block_extensions" data-list value="${esc(rules.block_extensions.join(', '))}">
    <span class="label">Hội thoại loại trừ</span><textarea rows="2" data-r="excluded_chats" data-lines placeholder="Mỗi dòng một tên hội thoại">${esc(rules.excluded_chats.join('\n'))}</textarea>
    <span class="label">Tự xóa tệp tự lưu sau (ngày)</span><input type="number" min="0" data-r="retention_days" value="${rules.retention_days}" style="width:120px">
    <span class="hint">0 = giữ mãi. Tệp bạn tự tải về không bao giờ bị tự xóa.</span>`;
  box.querySelectorAll('[data-r]').forEach((input) => input.addEventListener('change', () => {
    const k = input.dataset.r;
    if (input.type === 'checkbox') rules[k] = input.checked;
    else if (input.type === 'number') rules[k] = Math.max(0, parseInt(input.value || '0', 10) || 0);
    else if ('list' in input.dataset) rules[k] = input.value.split(/[,\s]+/).map((x) => x.replace(/^\./, '').trim().toLowerCase()).filter(Boolean);
    else if ('lines' in input.dataset) rules[k] = input.value.split('\n').map((x) => x.trim()).filter(Boolean);
    onChange();
  }));
  return box;
}

async function renderSettings(main) {
  const s = S.settings;
  const providers = S.providers;
  main.innerHTML = `<div class="page">
    <div class="page-head"><div class="grow"><h1>Cài đặt</h1><p class="muted">Mnemo ${esc(S.version)} · Every chat in sight, locked safe and tight.</p></div></div>

    <div class="section"><h2>Lưu trữ</h2><div class="card pad"><div class="form">
      <span class="label">Thư mục kho tin nhắn</span><div class="row"><span class="path grow mono">${esc(s.archive_dir)}</span><button class="btn sm" id="chg-archive">Đổi…</button><button class="btn sm ghost" data-folder="archive">${ICON.folder}</button></div>
      <span class="hint">Kho là một tệp mnemo.db mã hóa (SQLCipher). Khi đổi, Mnemo chép kho sang thư mục mới và giữ bản cũ.</span>
      <span class="label">Thư mục tệp tải về</span><div class="row"><span class="path grow mono">${esc(s.downloads_dir)}</span><button class="btn sm" id="chg-downloads">Đổi…</button><button class="btn sm ghost" data-folder="downloads">${ICON.folder}</button></div>
      <span class="hint">Tệp mới sẽ lưu ở thư mục mới; tệp cũ giữ nguyên chỗ.</span>
      <span class="label">Lưu tin nhắn từ các ứng dụng</span><label class="check"><span class="switch"><input type="checkbox" data-s="capture_enabled" ${s.capture_enabled ? 'checked' : ''}><span></span></span>Bật</label>
      <span class="label">Mã hóa tệp tự lưu</span><label class="check"><span class="switch"><input type="checkbox" data-s="encrypt_media" ${s.encrypt_media ? 'checked' : ''}><span></span></span>Bật (mở qua Mnemo)</label>
      <span class="label">Giữ tin bị thu hồi</span><label class="check"><span class="switch"><input type="checkbox" data-s="keep_recalled" ${s.keep_recalled ? 'checked' : ''}><span></span></span>Giữ bản đã lưu khi người gửi thu hồi</label>
      <span class="hint">Mặc định tắt: tin người khác đã thu hồi là quyền riêng tư của họ.</span>
      <span class="label">Khóa khôi phục</span><div class="row"><button class="btn sm" id="show-key">${ICON.lock} Hiện khóa</button><button class="btn sm" id="enter-key">Nhập khóa…</button></div>
      <span class="hint">Cần khóa này để mở kho trên máy khác. Hãy cất nó ở nơi an toàn (trình quản lý mật khẩu).</span>
    </div><div id="key-box"></div></div></div>

    <div class="section"><h2>Quy tắc tự động lưu tệp</h2><div class="card pad" id="rules"></div></div>

    <div class="section"><h2>AI</h2>
      <div class="card pad section"><div class="form">
        <span class="label">Mặc định cho trợ lý viết</span><div id="choice-writing"></div>
        <span class="label">Mặc định cho panel hội thoại</span><div id="choice-chat"></div>
        <span class="label">Tự kiểm tra khi ngừng gõ</span><label class="check"><span class="switch"><input type="checkbox" data-ai="auto_check" ${s.ai.auto_check ? 'checked' : ''}><span></span></span>Bật (tốn token)</label>
        <span class="hint">Khi tắt, bấm nút M cạnh con trỏ hoặc Ctrl+Shift+Space để mở trợ lý viết.</span>
        <span class="label">Số tin tối đa gửi cho AI</span><input type="number" min="20" max="5000" data-ai="max_context_messages" value="${s.ai.max_context_messages}" style="width:120px">
        <span class="label">Hướng dẫn chung cho AI</span><textarea rows="3" data-ai="personal_instructions" placeholder="vd: Tôi xưng 'mình', thích văn phong ngắn gọn, thân thiện.">${esc(s.ai.personal_instructions)}</textarea>
      </div></div>
      <h3 style="margin:14px 0 8px">Providers</h3>
      <div class="col" id="providers"></div>
      <div class="card pad section col" style="margin-top:12px">
        <b>Thêm API khác (tương thích OpenAI)</b>
        <div class="row wrap">
          <select id="preset"><option value="">Mẫu…</option>
            <option data-name="DeepSeek" data-url="https://api.deepseek.com/v1">DeepSeek</option>
            <option data-name="MiniMax" data-url="https://api.minimax.io/v1">MiniMax</option>
            <option data-name="OpenRouter" data-url="https://openrouter.ai/api/v1">OpenRouter</option>
            <option data-name="llama.cpp (máy này)" data-url="http://127.0.0.1:8080/v1">llama.cpp / máy chủ cục bộ</option>
          </select>
          <input type="text" id="np-name" placeholder="Tên" style="width:160px">
          <input type="text" id="np-url" class="grow" placeholder="https://api.example.com/v1">
          <button class="btn" id="np-add">${ICON.plus} Thêm</button>
        </div>
      </div>
      <h3 style="margin:14px 0 8px">Skills viết lách</h3>
      <div class="card pad"><div id="skills"></div>
        <div class="row" style="margin-top:10px"><button class="btn sm" id="sk-add">${ICON.plus} Thêm skill</button><button class="btn sm" id="sk-import">${ICON.file} Nhập từ tệp .md</button></div></div>
    </div>

    <div class="section"><h2>Thông báo</h2><div class="card pad"><div class="form">
      <span class="label">Thông báo tin nhắn mới</span><label class="check"><span class="switch"><input type="checkbox" data-n="enabled" ${s.notify.enabled ? 'checked' : ''}><span></span></span>Bật</label>
      <span class="hint">Hiện ở góc phải dưới màn hình: ứng dụng, người gửi, người nhận, thời gian, nội dung. Bấm vào để mở đúng hội thoại.</span>
      <span class="label">Hiện nội dung tin nhắn</span><label class="check"><span class="switch"><input type="checkbox" data-n="show_content" ${s.notify.show_content ? 'checked' : ''}><span></span></span>Bật</label>
      <span class="label">Thời gian hiển thị (giây)</span><input type="number" min="3" max="120" data-n="duration_secs" value="${s.notify.duration_secs}" style="width:120px">
      <span class="label">Thông báo theo tài khoản</span><div class="col">${accounts().map((a) => `<label class="check"><span class="switch"><input type="checkbox" data-acc-notify="${esc(a.id)}" ${a.notify ? 'checked' : ''}><span></span></span>${esc(SERVICE_NAME[a.service])} · ${esc(a.label)}</label>`).join('') || '<span class="muted small">Chưa có tài khoản</span>'}</div>
      <span class="hint">Chỉ tắt thông báo của Mnemo; tin nhắn vẫn được lưu và vẫn đếm chưa đọc.</span>
      <span class="label">Số tin chưa đọc trên taskbar</span><label class="check"><span class="switch"><input type="checkbox" data-n="taskbar_badge" ${s.notify.taskbar_badge ? 'checked' : ''}><span></span></span>Hiện tổng số của mọi tài khoản trên biểu tượng Mnemo</label>
    </div></div></div>

    <div class="section"><h2>Khởi động &amp; khay hệ thống</h2><div class="card pad"><div class="form">
      <span class="label">Khởi động cùng Windows</span><label class="check"><span class="switch"><input type="checkbox" id="autostart" ${s.start_with_windows ? 'checked' : ''}><span></span></span>Bật</label>
      <span class="hint">Mnemo chạy ẩn ở khay hệ thống (góc taskbar), không mở cửa sổ; tin nhắn vẫn được lưu và thông báo vẫn hiện.</span>
      <span class="label">Bấm × thì thu về khay</span><label class="check"><span class="switch"><input type="checkbox" data-s="close_to_tray" ${s.close_to_tray ? 'checked' : ''}><span></span></span>Bật</label>
      <span class="hint">Nút × chỉ ẩn cửa sổ. Thoát hẳn bằng chuột phải vào biểu tượng Mnemo ở khay → Thoát Mnemo.</span>
      <span class="label"></span><div><button class="btn danger" id="quit">${ICON.close} Thoát Mnemo ngay</button></div>
    </div></div></div>

    <div class="section"><h2>Giao diện</h2><div class="card pad"><div class="form">
      <span class="label">Ngôn ngữ</span><select data-s="language" style="width:200px" data-noi18n>
        ${[['vi', 'Tiếng Việt'], ['en', 'English']].map(([v, l]) => `<option value="${v}" ${s.language === v ? 'selected' : ''}>${l}</option>`).join('')}</select>
      <span class="label">Thanh bên Mnemo</span><select data-s="sidebar_right" style="width:200px">
        ${[['', 'Bên trái'], ['1', 'Bên phải']].map(([v, l]) => `<option value="${v}" ${!!s.sidebar_right === !!v ? 'selected' : ''}>${l}</option>`).join('')}</select>
      <span class="hint">Đặt bên phải để thanh điều hướng riêng của ứng dụng (WhatsApp, Discord…) nằm sát mép trái cửa sổ, không bị kẹp giữa hai thanh.</span>
      <span class="label">Chủ đề</span><select data-s="theme" style="width:200px">
        ${[['system', 'Theo Windows'], ['light', 'Sáng'], ['dark', 'Tối']].map(([v, l]) => `<option value="${v}" ${s.theme === v ? 'selected' : ''}>${l}</option>`).join('')}</select>
    </div></div></div>

    <div class="section"><h2>Nâng cao</h2><div class="card pad col">
      <div class="small muted">Các trang web của ứng dụng thay đổi theo thời gian. Nếu Mnemo không lưu được tin của một ứng dụng, có thể chỉnh selector trong thư mục cấu hình dịch vụ (chép <span class="mono">&lt;dịch vụ&gt;.default.json</span> thành <span class="mono">&lt;dịch vụ&gt;.json</span> rồi sửa), sau đó tải lại tài khoản. Dùng DevTools trong trang tài khoản để dò phần tử.</div>
      <div class="row"><button class="btn sm" data-folder="services">${ICON.folder} Cấu hình dịch vụ</button><button class="btn sm" data-folder="logs">${ICON.folder} Nhật ký</button><button class="btn sm" id="repo">GitHub</button></div>
    </div></div>
  </div>`;

  main.querySelectorAll('[data-s]').forEach((input) => input.addEventListener('change', () => {
    s[input.dataset.s] = input.type === 'checkbox' ? input.checked : input.value;
    if (input.dataset.s === 'theme') applyTheme(s.theme);
    if (input.dataset.s === 'sidebar_right') {
      s.sidebar_right = !!input.value;
      syncLayout().then(renderSidebar);
    }
    if (input.dataset.s === 'language') {
      setLang(s.language);
      renderSidebar();
      renderMain();
      if (panelOpen) renderPanel();
    }
    saveSettingsSoon();
  }));
  main.querySelectorAll('[data-acc-notify]').forEach((input) => input.addEventListener('change', async () => {
    const a = accountById(input.dataset.accNotify);
    if (!a) return;
    a.notify = input.checked;
    try { await invoke('update_account', { account: a }); } catch (e) { toast(errText(e), true); }
  }));
  $('#autostart').addEventListener('change', async (e) => {
    try { await invoke('set_autostart', { enabled: e.target.checked }); s.start_with_windows = e.target.checked; } catch (err) { e.target.checked = !e.target.checked; toast(errText(err), true); }
  });
  $('#quit').addEventListener('click', () => invoke('quit_app'));
  main.querySelectorAll('[data-n]').forEach((input) => input.addEventListener('change', () => {
    const k = input.dataset.n;
    s.notify[k] = input.type === 'checkbox' ? input.checked : Math.min(120, Math.max(3, parseInt(input.value, 10) || 8));
    saveSettingsSoon();
  }));
  main.querySelectorAll('[data-ai]').forEach((input) => input.addEventListener('change', () => {
    const k = input.dataset.ai;
    s.ai[k] = input.type === 'checkbox' ? input.checked : input.type === 'number' ? parseInt(input.value, 10) || 400 : input.value;
    saveSettingsSoon();
  }));
  main.querySelectorAll('[data-folder]').forEach((b) => b.addEventListener('click', () => invoke('open_folder', { which: b.dataset.folder }).catch((e) => toast(errText(e), true))));
  $('#repo').addEventListener('click', () => invoke('open_url', { url: 'https://github.com/nev-minhnh/Mnemo' }));
  $('#rules').appendChild(rulesEditor(s.rules, saveSettingsSoon));
  choiceBar($('#choice-writing'), s.ai.writing, providers, saveSettingsSoon);
  choiceBar($('#choice-chat'), s.ai.chat, providers, saveSettingsSoon);

  $('#chg-archive').addEventListener('click', async () => {
    const path = await invoke('pick_folder', { title: 'Chọn thư mục kho tin nhắn' });
    if (!path) return;
    try { toast(await invoke('set_archive_dir', { path })); await refresh(); renderSettings(main); } catch (e) { toast(errText(e), true); }
  });
  $('#chg-downloads').addEventListener('click', async () => {
    const path = await invoke('pick_folder', { title: 'Chọn thư mục lưu tệp' });
    if (!path) return;
    try { await invoke('set_downloads_dir', { path }); await refresh(); renderSettings(main); } catch (e) { toast(errText(e), true); }
  });
  $('#show-key').addEventListener('click', async () => {
    try {
      const key = await invoke('reveal_recovery_key');
      $('#key-box').innerHTML = `<div class="notice warn" style="margin-top:12px">Ai có khóa này và tệp mnemo.db đều đọc được toàn bộ tin nhắn.<div class="mono" style="margin-top:6px;user-select:all">${esc(key)}</div></div>`;
    } catch (e) { toast(errText(e), true); }
  });
  $('#enter-key').addEventListener('click', () => {
    $('#key-box').innerHTML = `<div class="row" style="margin-top:12px"><input type="text" id="rk" class="grow mono" placeholder="XXXXXXXX-XXXXXXXX-…"><button class="btn primary" id="rk-ok">Mở kho bằng khóa này</button></div>`;
    $('#rk-ok').addEventListener('click', async () => {
      try { await invoke('import_recovery_key', { key: $('#rk').value }); toast('Đã mở kho'); await refresh(); renderSettings(main); } catch (e) { toast(errText(e), true); }
    });
  });

  renderProviders();
  renderSkills();
  $('#preset').addEventListener('change', (e) => {
    const o = e.target.selectedOptions[0];
    if (o && o.dataset.url) { $('#np-name').value = o.dataset.name; $('#np-url').value = o.dataset.url; }
  });
  $('#np-add').addEventListener('click', async () => {
    try {
      await invoke('add_provider', { name: $('#np-name').value, baseUrl: $('#np-url').value });
      await refresh();
      renderSettings(main);
    } catch (e) { toast(errText(e), true); }
  });
  $('#sk-add').addEventListener('click', () => {
    s.ai.skills.push({ id: Math.random().toString(16).slice(2, 14), name: 'Skill mới', instruction: '', builtin: false });
    saveSettingsSoon();
    renderSkills();
  });
  $('#sk-import').addEventListener('click', async () => {
    const path = await invoke('pick_file', { title: 'Chọn tệp skill (SKILL.md)', extensions: ['md', 'txt'] });
    if (!path) return;
    try { await invoke('ai_import_skill', { path }); await refresh(); renderSettings(main); } catch (e) { toast(errText(e), true); }
  });
}

const KIND_LABEL = {
  claude_code: 'CLI · tài khoản Claude', codex_cli: 'CLI · tài khoản ChatGPT', openai: 'API key', anthropic: 'API key',
  gemini: 'API key (Google AI Studio)', xai: 'API key', compatible: 'OpenAI-compatible',
};

function renderProviders() {
  const box = $('#providers');
  if (!box) return;
  const cfgs = S.settings.ai.providers;
  box.innerHTML = '';
  for (const p of cfgs) {
    const view = S.providers.find((v) => v.id === p.id) || {};
    const isCli = p.kind === 'claude_code' || p.kind === 'codex_cli';
    const custom = p.id.startsWith('custom-');
    const card = el(`<div class="card provider">
      <div class="row">
        <b class="grow">${esc(p.name)}</b>
        <span class="badge">${KIND_LABEL[p.kind] || p.kind}</span>
        ${isCli ? '' : view.has_key ? '<span class="badge ok">Đã có key</span>' : p.kind === 'compatible' ? '' : '<span class="badge warn">Chưa có key</span>'}
        <label class="check small"><span class="switch"><input type="checkbox" data-en ${p.enabled ? 'checked' : ''}><span></span></span>Bật</label>
      </div>
      ${isCli ? `<div class="row"><input type="text" class="grow mono" data-cli placeholder="${window.LANG === 'en' ? `Finds ${p.kind === 'claude_code' ? 'claude' : 'codex'} on this computer (or enter the full path)` : `Tự tìm ${p.kind === 'claude_code' ? 'claude' : 'codex'} trên máy (hoặc nhập đường dẫn đầy đủ)`}" value="${esc(p.cli_path)}"></div>
        <div class="small muted">${window.LANG === 'en' ? `Uses the account you signed in with in ${p.kind === 'claude_code' ? 'Claude Code (the <span class="mono">claude</span> command)' : 'Codex CLI (<span class="mono">codex login</span>)'}.` : `Dùng chính tài khoản bạn đã đăng nhập trong ${p.kind === 'claude_code' ? 'Claude Code (lệnh <span class="mono">claude</span>)' : 'Codex CLI (lệnh <span class="mono">codex login</span>)'}.`}</div>`
      : `<div class="row"><input type="password" class="grow" data-key placeholder="${view.has_key ? '•••••••• (đã lưu – nhập để thay)' : 'Dán API key'}"><button class="btn sm" data-savekey>Lưu key</button>${view.has_key ? '<button class="btn sm ghost danger" data-delkey>Xóa key</button>' : ''}</div>
        ${p.kind === 'compatible' || custom ? `<div class="row"><input type="text" class="grow mono" data-url value="${esc(p.base_url)}"></div>` : ''}`}
      <div class="row wrap">
        <button class="btn sm" data-models>Tải danh sách model</button>
        <button class="btn sm" data-test>Thử</button>
        ${custom ? '<button class="btn sm ghost danger" data-remove>Xóa provider</button>' : ''}
        <span class="small muted grow ellipsis" data-info>${p.models.length ? `${p.models.length} model: ${esc(p.models.slice(0, 6).join(', '))}${p.models.length > 6 ? '…' : ''}` : ''}</span>
      </div>
    </div>`);
    const update = async (patch) => {
      Object.assign(p, patch);
      try { await invoke('update_provider', { provider: p }); await refresh(); } catch (e) { toast(errText(e), true); }
    };
    card.querySelector('[data-en]').addEventListener('change', (e) => update({ enabled: e.target.checked }));
    const cli = card.querySelector('[data-cli]');
    if (cli) cli.addEventListener('change', () => update({ cli_path: cli.value.trim() }));
    const url = card.querySelector('[data-url]');
    if (url) url.addEventListener('change', () => update({ base_url: url.value.trim() }));
    const sk = card.querySelector('[data-savekey]');
    if (sk) sk.addEventListener('click', async () => {
      const key = card.querySelector('[data-key]').value.trim();
      if (!key) return;
      try { await invoke('ai_set_key', { providerId: p.id, key }); toast('Đã lưu key (mã hóa bằng Windows)'); await refresh(); renderProviders(); } catch (e) { toast(errText(e), true); }
    });
    const dk = card.querySelector('[data-delkey]');
    if (dk) dk.addEventListener('click', async () => { await invoke('ai_set_key', { providerId: p.id, key: null }); await refresh(); renderProviders(); });
    const rm = card.querySelector('[data-remove]');
    if (rm) rm.addEventListener('click', async () => { try { await invoke('remove_provider', { id: p.id }); await refresh(); renderProviders(); } catch (e) { toast(errText(e), true); } });
    card.querySelector('[data-models]').addEventListener('click', async (e) => {
      const info = card.querySelector('[data-info]');
      info.textContent = 'Đang tải…';
      try {
        const models = await invoke('ai_list_models', { providerId: p.id });
        await refresh();
        info.textContent = models.length ? `${models.length} model: ${models.slice(0, 6).join(', ')}${models.length > 6 ? '…' : ''}` : 'Provider không trả về danh sách; hãy gõ tên model trực tiếp.';
      } catch (err) { info.textContent = errText(err); }
    });
    card.querySelector('[data-test]').addEventListener('click', async () => {
      const info = card.querySelector('[data-info]');
      info.textContent = 'Đang thử…';
      try {
        const model = isCli ? '' : (p.models[0] || '');
        const out = await invoke('ai_test', { choice: { provider: p.id, model, effort: '', thinking: false } });
        info.textContent = '✓ ' + out;
      } catch (err) { info.textContent = errText(err); }
    });
    box.appendChild(card);
  }
}

function renderSkills() {
  const box = $('#skills');
  if (!box) return;
  const skills = S.settings.ai.skills;
  box.innerHTML = skills.map((k, i) => `<div class="skill" data-i="${i}">
    <input type="text" data-f="name" value="${esc(k.name)}">
    <textarea rows="2" data-f="instruction" placeholder="Hướng dẫn cho AI, vd: Viết lại cho lịch sự, xưng em, gọi anh/chị.">${esc(k.instruction)}</textarea>
    <button class="btn sm ghost danger" data-del title="Xóa">${ICON.close}</button>
  </div>`).join('') || '<div class="muted small">Chưa có skill nào.</div>';
  box.querySelectorAll('.skill').forEach((row) => {
    const k = skills[+row.dataset.i];
    row.querySelectorAll('[data-f]').forEach((input) => input.addEventListener('change', () => { k[input.dataset.f] = input.value; saveSettingsSoon(); }));
    row.querySelector('[data-del]').addEventListener('click', () => { skills.splice(+row.dataset.i, 1); saveSettingsSoon(); renderSkills(); });
  });
}

// ---------------------------------------------------------------- account page

function renderAccount(main, id) {
  const a = accountById(id);
  if (!a) { openPage('home'); return; }
  const stats = (S.stats || {})[a.id] || {};
  main.innerHTML = `<div class="page">
    <div class="page-head">${avatarHtml(a, 56)}<div class="grow"><p class="muted" style="margin:0">${SERVICE_NAME[a.service]}</p><h1 data-noi18n>${esc(a.label)}</h1></div>
      <button class="btn primary" id="acc-open">Mở</button></div>
    <div class="section"><div class="card pad"><div class="form">
      <span class="label">Tên hiển thị</span><input type="text" data-a="label" value="${esc(a.label)}" maxlength="60">
      <span class="label">Ảnh đại diện</span><div class="row"><button class="btn sm" id="pick-avatar">Chọn ảnh…</button><button class="btn sm ghost" id="auto-avatar">Dùng ảnh tự động</button></div>
      <span class="hint">Mnemo tự lấy ảnh đại diện của bạn từ ${SERVICE_NAME[a.service]} khi trang cho phép; nếu không, hãy chọn ảnh.</span>
      <span class="label">Tên của bạn trong ${SERVICE_NAME[a.service]}</span><input type="text" data-a="self_name" value="${esc(a.self_name)}" placeholder="vd: Minh Nguyễn">
      <span class="hint">Dùng để nhận ra tin của bạn khi nhập tệp xuất và khi AI gợi ý câu trả lời.</span>
      <span class="label">Thông báo của Mnemo</span><label class="check"><span class="switch"><input type="checkbox" data-a="notify" ${a.notify ? 'checked' : ''}><span></span></span>Hiện thông báo tin mới của tài khoản này</label>
      <span class="label">Lưu tin nhắn của tài khoản này</span><label class="check"><span class="switch"><input type="checkbox" data-a="capture" ${a.capture ? 'checked' : ''}><span></span></span>Bật</label>
      <span class="label">Phiên này</span><span class="muted">${stats.messages || 0} tin, ${stats.media || 0} tệp đã lưu${stats.last_error ? ` · lỗi gần nhất: ${esc(stats.last_error)}` : ''}</span>
      <span class="label">Quy tắc lưu tệp riêng</span><label class="check"><span class="switch"><input type="checkbox" id="own-rules" ${a.rules ? 'checked' : ''}><span></span></span>Dùng quy tắc riêng cho tài khoản này</label>
    </div><div id="acc-rules" style="margin-top:14px"></div></div></div>
    <div class="section"><h2>Trang của tài khoản</h2><div class="card pad row wrap">
      <button class="btn" id="acc-reload">Tải lại</button>
      <button class="btn" id="acc-sleep" title="Đóng trang để giải phóng bộ nhớ; mở lại khi bấm vào">Cho ngủ</button>
      <button class="btn" id="acc-devtools">DevTools</button>
      <button class="btn" id="acc-up">↑ Lên</button><button class="btn" id="acc-down">↓ Xuống</button>
    </div></div>
    <div class="section"><h2>Xóa tài khoản</h2><div class="card pad col">
      <div class="small muted">Tin nhắn và tệp đã lưu vẫn ở trong kho. Nếu chọn xóa dữ liệu đăng nhập, lần sau phải đăng nhập lại.</div>
      <div class="row"><label class="check"><input type="checkbox" id="wipe">Xóa cả dữ liệu đăng nhập của tài khoản này</label><span class="grow"></span><button class="btn danger" id="acc-remove">Xóa tài khoản</button></div>
    </div></div>
  </div>`;
  const save = async () => { try { await invoke('update_account', { account: a }); await refresh(); } catch (e) { toast(errText(e), true); } };
  main.querySelectorAll('[data-a]').forEach((input) => input.addEventListener('change', () => {
    a[input.dataset.a] = input.type === 'checkbox' ? input.checked : input.value;
    save();
  }));
  const drawRules = () => {
    const box = $('#acc-rules');
    box.innerHTML = '';
    if (a.rules) box.appendChild(rulesEditor(a.rules, save));
  };
  $('#own-rules').addEventListener('change', (e) => {
    a.rules = e.target.checked ? JSON.parse(JSON.stringify(S.settings.rules)) : null;
    drawRules();
    save();
  });
  drawRules();
  $('#acc-open').addEventListener('click', () => showAccount(a.id));
  $('#pick-avatar').addEventListener('click', async () => {
    const path = await invoke('pick_file', { title: 'Chọn ảnh đại diện', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] });
    if (!path) return;
    try { await invoke('set_account_avatar', { id: a.id, path }); await refresh(); renderAccount(main, id); } catch (e) { toast(errText(e), true); }
  });
  $('#auto-avatar').addEventListener('click', async () => {
    try { await invoke('set_account_avatar', { id: a.id, path: null }); await refresh(); renderAccount(main, id); toast('Sẽ lấy lại ảnh từ trang khi có'); } catch (e) { toast(errText(e), true); }
  });
  $('#acc-reload').addEventListener('click', () => invoke('reload_account', { id: a.id }).then(() => toast('Đã tải lại')).catch((e) => toast(errText(e), true)));
  $('#acc-sleep').addEventListener('click', () => invoke('sleep_account', { id: a.id }).then(() => toast('Đã cho ngủ')));
  $('#acc-devtools').addEventListener('click', async () => { try { await invoke('show_account', { id: a.id }); visible = a.id; renderSidebar(); await invoke('open_devtools', { id: a.id }); } catch (e) { toast(errText(e), true); } });
  $('#acc-up').addEventListener('click', async () => { await invoke('move_account', { id: a.id, delta: -1 }); await refresh(); });
  $('#acc-down').addEventListener('click', async () => { await invoke('move_account', { id: a.id, delta: 1 }); await refresh(); });
  $('#acc-remove').addEventListener('click', async () => {
    if (!confirm(`Xóa tài khoản "${a.label}" khỏi Mnemo?`)) return;
    try { await invoke('remove_account', { id: a.id, wipeProfile: $('#wipe').checked }); await refresh(); openPage('home'); } catch (e) { toast(errText(e), true); }
  });
}

// ---------------------------------------------------------------- AI panel

const panel = { tab: 'one', chat: null, result: null, replies: null, busy: false, error: null, multi: new Set(), qa: [], chats: [], from: '', to: '', hint: '' };

async function togglePanel(force) {
  panelOpen = force === undefined ? !panelOpen : force;
  $('#app').classList.toggle('panel', panelOpen);
  $('#panel').classList.toggle('hidden', !panelOpen);
  await syncLayout();
  renderSidebar();
  if (panelOpen) renderPanel();
}

async function panelTarget() {
  if (visible) {
    const chat = await invoke('live_chat', { accountId: visible }).catch(() => null);
    return { account: accountById(visible), chat };
  }
  if (page === 'archive' && archive.chat) return { account: accountById(archive.chat.account), chat: archive.chat };
  return { account: null, chat: null };
}

function toLocalInput(ms) {
  const d = new Date(ms - new Date().getTimezoneOffset() * 60000);
  return d.toISOString().slice(0, 16);
}

async function renderPanel() {
  const box = $('#panel');
  if (!panelOpen || !S) return;
  const target = await panelTarget();
  if (!panel.chat || !target.chat || panel.chat.id !== target.chat.id) { panel.result = null; panel.replies = null; panel.error = null; }
  panel.chat = target.chat;
  const service = target.account ? target.account.service : (target.chat ? target.chat.service : null);
  if (!panel.from) { panel.from = toLocalInput(Date.now() - 24 * 3600 * 1000); panel.to = toLocalInput(Date.now()); }
  box.innerHTML = `
    <div class="panel-head">${ICON.sparkle}<b class="grow">Trợ lý AI</b><button class="btn icon ghost" id="pclose" title="Đóng">${ICON.close}</button></div>
    <div class="panel-body scroll">
      <div id="pchoice"></div>
      <div class="tabs"><button data-tab="one" class="${panel.tab === 'one' ? 'active' : ''}">Hội thoại này</button><button data-tab="multi" class="${panel.tab === 'multi' ? 'active' : ''}">Nhiều hội thoại</button></div>
      <div id="ptab"></div>
    </div>`;
  $('#pclose').addEventListener('click', () => togglePanel(false));
  choiceBar($('#pchoice'), S.settings.ai.chat, S.providers, (c) => invoke('assist_save_choice', { choice: c, panel: true }));
  box.querySelectorAll('[data-tab]').forEach((b) => b.addEventListener('click', () => { panel.tab = b.dataset.tab; renderPanel(); }));
  if (panel.tab === 'one') renderPanelOne($('#ptab'), target);
  else renderPanelMulti($('#ptab'), service, target.account);
}

function renderPanelOne(box, target) {
  const c = panel.chat;
  if (!c) {
    box.innerHTML = `<div class="notice">${target.account
      ? 'Mở một hội thoại trong ứng dụng. Mnemo nhận ra hội thoại khi tin nhắn của nó đã được lưu.'
      : 'Mở một tài khoản hoặc chọn một hội thoại trong Kho tin nhắn.'}</div>`;
    return;
  }
  box.innerHTML = `
    <div class="row">${svcIcon(c.service, 28)}<div class="grow"><b class="ellipsis">${esc(c.title || c.chat_key)}</b><div class="small muted">${c.count} tin đã lưu · ${c.unseen} chưa xem</div></div></div>
    <div class="col">
      <button class="btn primary" id="sum-unseen" ${panel.busy ? 'disabled' : ''}>Tóm tắt tin chưa xem${c.unseen ? ` (${c.unseen})` : ''}</button>
      <div class="range"><label>Từ<input type="datetime-local" id="pfrom" value="${panel.from}"></label><label>Đến<input type="datetime-local" id="pto" value="${panel.to}"></label></div>
      <button class="btn" id="sum-range" ${panel.busy ? 'disabled' : ''}>Tóm tắt theo khoảng thời gian</button>
      <div class="row"><input type="text" id="phint" class="grow" placeholder="Ý muốn trả lời (không bắt buộc)" value="${esc(panel.hint)}"><button class="btn" id="suggest" ${panel.busy ? 'disabled' : ''}>Gợi ý trả lời</button></div>
      <button class="btn sm ghost" id="pseen">Đánh dấu đã xem tất cả</button>
    </div>
    ${panel.busy ? '<div class="row muted small"><div class="spinner"></div>AI đang xử lý…</div>' : ''}
    ${panel.error ? `<div class="notice danger">${esc(panel.error)}</div>` : ''}
    ${panel.result ? `<div class="result">${md(panel.result.text)}<div class="small faint" style="margin-top:6px">${panel.result.count} tin${panel.result.truncated ? ' · đã cắt bớt (tăng "Số tin tối đa gửi cho AI" trong Cài đặt)' : ''}</div></div>` : ''}
    ${panel.replies ? `<div class="col">${panel.replies.map((r, i) => `<div class="reply"><div class="row"><span class="badge accent">${esc(r.label || 'Gợi ý')}</span></div><div class="text" data-noi18n>${esc(r.text)}</div>
      <div class="row">${visible ? `<button class="btn sm primary" data-insert="${i}">Chèn vào ô soạn tin</button>` : ''}<button class="btn sm" data-copy="${i}">Sao chép</button></div></div>`).join('')}</div>` : ''}`;
  $('#pfrom').addEventListener('change', (e) => { panel.from = e.target.value; });
  $('#pto').addEventListener('change', (e) => { panel.to = e.target.value; });
  $('#phint').addEventListener('change', (e) => { panel.hint = e.target.value; });
  const run = async (fn) => {
    panel.busy = true; panel.error = null; renderPanelOne(box, target);
    try { await fn(); } catch (e) { panel.error = errText(e); }
    panel.busy = false; renderPanelOne(box, target);
  };
  $('#sum-unseen').addEventListener('click', () => run(async () => {
    panel.replies = null;
    panel.result = await invoke('ai_summarize', { chatId: c.id, mode: 'unseen', from: null, to: null, choice: S.settings.ai.chat, focus: null });
  }));
  $('#sum-range').addEventListener('click', () => run(async () => {
    panel.replies = null;
    const from = new Date(panel.from).getTime();
    const to = new Date(panel.to).getTime();
    if (!(from < to)) throw new Error('Khoảng thời gian không hợp lệ');
    panel.result = await invoke('ai_summarize', { chatId: c.id, mode: 'range', from, to, choice: S.settings.ai.chat, focus: null });
  }));
  $('#suggest').addEventListener('click', () => run(async () => {
    const r = await invoke('ai_suggest_replies', { chatId: c.id, choice: S.settings.ai.chat, hint: panel.hint || null });
    panel.replies = Array.isArray(r.replies) ? r.replies.filter((x) => x && x.text) : [];
    if (!panel.replies.length) throw new Error('AI không đưa ra gợi ý nào');
  }));
  $('#pseen').addEventListener('click', async () => { await invoke('mark_seen', { chatId: c.id }); renderPanel(); });
  box.querySelectorAll('[data-insert]').forEach((b) => b.addEventListener('click', async () => {
    try { await invoke('ai_insert_reply', { accountId: visible, text: panel.replies[+b.dataset.insert].text }); } catch (e) { toast(errText(e), true); }
  }));
  box.querySelectorAll('[data-copy]').forEach((b) => b.addEventListener('click', () => navigator.clipboard.writeText(panel.replies[+b.dataset.copy].text)));
}

async function renderPanelMulti(box, service, account) {
  panel.chats = await invoke('list_chats', { service: service || null, account: account ? account.id : null, query: null }).catch(() => []);
  box.innerHTML = `
    <div class="small muted">Chọn các hội thoại${service ? ` ${SERVICE_NAME[service]}` : ''} rồi đặt câu hỏi, vd: "Tuần này ai hẹn gặp tôi?", "Tổng hợp các khoản cần thanh toán".</div>
    <input type="search" id="mfilter" placeholder="Lọc hội thoại">
    <div class="pick-list scroll" id="mlist"></div>
    <div class="row small"><button class="btn sm ghost" id="mall">Chọn tất cả</button><button class="btn sm ghost" id="mnone">Bỏ chọn</button><span class="grow"></span><span class="faint" id="mcount"></span></div>
    <div class="range"><label>Từ<input type="datetime-local" id="mfrom" value="${panel.from}"></label><label>Đến<input type="datetime-local" id="mto" value="${panel.to}"></label></div>
    <div class="qa" id="qa"></div>
    <div class="row"><textarea id="mq" rows="2" class="grow" placeholder="Hỏi về các hội thoại đã chọn… (Enter để gửi)"></textarea><button class="btn primary" id="msend">Gửi</button></div>
    ${panel.qa.length ? '<button class="btn sm ghost" id="mclear">Xóa cuộc trò chuyện</button>' : ''}`;
  const drawList = (filter) => {
    const f = (filter || '').toLowerCase();
    $('#mlist').innerHTML = panel.chats.filter((c) => !f || (c.title || '').toLowerCase().includes(f)).map((c) => `<label>
      <input type="checkbox" data-c="${c.id}" ${panel.multi.has(c.id) ? 'checked' : ''}>${svcIcon(c.service, 20)}<span class="ellipsis grow" data-noi18n>${esc(c.title || c.chat_key)}</span><span class="faint small">${c.count}</span></label>`).join('') || '<div class="empty small">Chưa có hội thoại</div>';
    $('#mlist').querySelectorAll('[data-c]').forEach((cb) => cb.addEventListener('change', () => {
      const id = +cb.dataset.c;
      if (cb.checked) panel.multi.add(id); else panel.multi.delete(id);
      $('#mcount').textContent = `${panel.multi.size} đã chọn`;
    }));
    $('#mcount').textContent = `${panel.multi.size} đã chọn`;
  };
  const drawQa = () => {
    $('#qa').innerHTML = panel.qa.map((t) => t.role === 'user' ? `<div class="q" data-noi18n>${esc(t.content)}</div>` : `<div class="a">${t.error ? `<span style="color:var(--danger)">${esc(t.content)}</span>` : md(t.content)}</div>`).join('')
      + (panel.busy ? '<div class="row muted small"><div class="spinner"></div>AI đang đọc các hội thoại…</div>' : '');
  };
  drawList('');
  drawQa();
  $('#mfilter').addEventListener('input', (e) => drawList(e.target.value));
  $('#mall').addEventListener('click', () => { panel.chats.forEach((c) => panel.multi.add(c.id)); drawList($('#mfilter').value); });
  $('#mnone').addEventListener('click', () => { panel.multi.clear(); drawList($('#mfilter').value); });
  $('#mfrom').addEventListener('change', (e) => { panel.from = e.target.value; });
  $('#mto').addEventListener('change', (e) => { panel.to = e.target.value; });
  const clear = $('#mclear');
  if (clear) clear.addEventListener('click', () => { panel.qa = []; renderPanel(); });
  const send = async () => {
    const q = $('#mq').value.trim();
    if (!q || panel.busy) return;
    if (!panel.multi.size) { toast('Hãy chọn ít nhất một hội thoại', true); return; }
    const history = panel.qa.filter((t) => !t.error).map((t) => ({ role: t.role, content: t.content }));
    panel.qa.push({ role: 'user', content: q });
    $('#mq').value = '';
    panel.busy = true; drawQa();
    try {
      const r = await invoke('ai_ask', {
        chatIds: Array.from(panel.multi), question: q, history,
        from: panel.from ? new Date(panel.from).getTime() : null, to: panel.to ? new Date(panel.to).getTime() : null,
        choice: S.settings.ai.chat,
      });
      panel.qa.push({ role: 'assistant', content: r.text + (r.truncated ? '\n\n(đã bỏ bớt tin cũ cho vừa giới hạn)' : '') });
    } catch (e) {
      panel.qa.push({ role: 'assistant', content: errText(e), error: true });
    }
    panel.busy = false; drawQa();
  };
  $('#msend').addEventListener('click', send);
  $('#mq').addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); } });
}

// ---------------------------------------------------------------- events & start

async function start() {
  try {
    await refresh();
  } catch (e) {
    $('#main').innerHTML = `<div class="page"><div class="notice danger">${esc(errText(e))}</div></div>`;
    return;
  }
  panelOpen = !!S.panelOpen;
  await syncLayout();
  $('#app').classList.toggle('panel', panelOpen);
  $('#panel').classList.toggle('hidden', !panelOpen);
  if (!accounts().length) page = 'home';
  await renderMain();
  if (panelOpen) renderPanel();

  listen('unread', (e) => {
    if (!S) return;
    S.unread[e.payload.account] = e.payload.count;
    renderSidebar();
  });
  listen('visible', (e) => {
    visible = e.payload.account;
    renderSidebar();
    if (panelOpen) renderPanel();
  });
  let ctxTimer = null;
  listen('context', (e) => {
    if (!panelOpen || e.payload.account !== visible || panel.tab !== 'one') return;
    clearTimeout(ctxTimer);
    ctxTimer = setTimeout(renderPanel, 600);
  });
  listen('captured', (e) => {
    if (!panelOpen || e.payload.account !== visible || panel.busy || panel.tab !== 'one') return;
    if (document.activeElement && document.activeElement.closest && document.activeElement.closest('#panel')) return;
    clearTimeout(ctxTimer);
    ctxTimer = setTimeout(renderPanel, 1500);
  });
  listen('import-progress', (e) => {
    importState.progress = e.payload;
    const n = $('#imp-progress');
    if (n) n.textContent = `${e.payload.chats} hội thoại · ${e.payload.messages} tin · ${e.payload.media} tệp — ${e.payload.current}`;
  });
  listen('accounts', async () => {
    await refresh();
    if (page === 'home' && !visible) renderMain();
  });
  listen('icons', (e) => {
    window.SVC_ICONS = e.payload || {};
    if (S) S.icons = window.SVC_ICONS;
    renderSidebar();
    if (page === 'home' && !visible) renderMain();
  });
  listen('download', (e) => toast(`Đã tải về: ${e.payload.name}`));
  listen('avatar', (e) => {
    if (!S) return;
    if (e.payload.avatar) S.avatars[e.payload.account] = e.payload.avatar; else delete S.avatars[e.payload.account];
    renderSidebar();
  });
}

start();
