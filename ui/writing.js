// The Writing assistant panel: it shares the right-hand panel slot with the AI assistant.
// It shows the check of the draft in the open account's message box (reported by the page)
// and acts on it there: accept, dismiss, add to dictionary, highlight. Only that draft is
// ever checked; the conversation is not read.
'use strict';

const W = { account: null, data: null, tab: 'list', filter: null, open: null, known: new Set(), text: null, busy: false };
const W_CAT = [
  ['correctness', 'Đúng', '#f03c45'],
  ['clarity', 'Rõ ràng', '#1a8cf8'],
  ['engagement', 'Hấp dẫn', '#2bb86a'],
  ['delivery', 'Giọng điệu', '#8b5cf6'],
];
const W_COLOR = Object.fromEntries(W_CAT.map(([k, , c]) => [k, c]));
const W_GOALS = {
  formality: ['Mức trang trọng', [['casual', 'Thân mật'], ['neutral', 'Trung tính'], ['formal', 'Trang trọng']]],
  audience: ['Người nhận', [['general', 'Chung'], ['friends', 'Bạn bè'], ['family', 'Gia đình'], ['colleagues', 'Đồng nghiệp'], ['customers', 'Khách hàng'], ['boss', 'Cấp trên']]],
  intent: ['Mục đích', [['chat', 'Trò chuyện'], ['inform', 'Thông báo'], ['request', 'Nhờ / đề nghị'], ['persuade', 'Thuyết phục'], ['apologise', 'Xin lỗi'], ['thank', 'Cảm ơn']]],
  variant: ['Ngôn ngữ', [['auto', 'Theo bản nháp'], ['vi', 'Tiếng Việt'], ['en-US', 'Tiếng Anh (Mỹ)'], ['en-GB', 'Tiếng Anh (Anh)']]],
};
const W_WRITE = [
  ['Trang trọng hơn', 'Rewrite it in a more formal, professional tone.'],
  ['Thân thiện hơn', 'Rewrite it in a warmer, friendlier tone.'],
  ['Ngắn gọn hơn', 'Make it more concise without losing meaning.'],
  ['Mở rộng', 'Expand it with a little more detail, still as a chat message.'],
  ['Đơn giản hơn', 'Rewrite it in simpler words and shorter sentences.'],
  ['Hay hơn', 'Improve its clarity, flow and word choice.'],
];
const W_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 17.5 7.6 6.5 11.2 17.5M5.3 13.6h4.6"/><path d="m13.5 13.8 2.6 2.7 4.7-6"/></svg>';

function wRing(score, busy) {
  const v = score == null ? 0 : Math.max(0, Math.min(100, Math.round(score)));
  const color = v >= 90 ? 'var(--ok)' : v >= 70 ? 'var(--gold-2)' : 'var(--danger)';
  const c = 2 * Math.PI * 24;
  return `<div class="w-ring ${busy ? 'busy' : ''}"><svg viewBox="0 0 56 56"><circle cx="28" cy="28" r="24" fill="none" stroke="var(--surface-3)" stroke-width="5"/>
    <circle class="arc" cx="28" cy="28" r="24" fill="none" stroke="${color}" stroke-width="5" stroke-linecap="round" stroke-dasharray="${c}" stroke-dashoffset="${busy || score == null ? c * 0.75 : c * (1 - v / 100)}"/></svg>
    <b>${busy ? '' : score == null ? '–' : v}</b></div>`;
}

async function openWriting(account) {
  account = account || null;
  W.account = account;
  W.tab = 'list';
  W.filter = null;
  W.open = null;
  W.known = new Set();
  W.text = null;
  W.data = null;
  if (account) {
    try { W.data = await invoke('writing_state', { account }); } catch (e) { W.data = null; }
    // Ask the page for its draft and check it if it hasn't been checked yet.
    invoke('writing_action', { account, action: 'open', original: null }).catch(() => {});
  }
  const switching = panel.mode !== 'writing';
  panel.mode = 'writing';
  if (!panelOpen) await togglePanel(true);
  else { if (switching) enter($('#panel')); renderPanel(); renderSidebar(); }
}

function writingUpdate(v) {
  if (v.account !== W.account) return;
  W.data = Object.assign(W.data || {}, v);
  if (panelOpen && panel.mode === 'writing') renderWritingBody();
}

function renderWriting(box) {
  const a = accountById(W.account);
  box.innerHTML = `
    <div class="panel-head w-head"><span class="w-logo">${W_ICON}</span><b class="grow">Trợ lý viết</b>
      ${a ? `<span class="small muted ellipsis" data-noi18n style="max-width:120px">${esc(a.label)}</span>` : ''}
      <button class="btn icon ghost" id="pclose" title="Đóng">${ICON.close}</button></div>
    <div class="panel-body scroll" id="wbody"></div>`;
  $('#pclose').addEventListener('click', () => togglePanel(false));
  renderWritingBody();
}

function renderWritingBody() {
  const body = $('#wbody');
  if (!body) return;
  const d = W.data;
  if (!W.account) { body.innerHTML = '<div class="notice">Mở một tài khoản và gõ vào ô soạn tin: Trợ lý viết kiểm tra đúng nội dung trong ô đó, không đọc cuộc trò chuyện.</div>'; return; }
  if (!d) { body.innerHTML = '<div class="row muted small"><div class="spinner"></div>Đang đọc ô soạn tin…</div>'; return; }
  const issues = d.issues || [];
  const counts = Object.fromEntries(W_CAT.map(([k]) => [k, issues.filter((i) => i.category === k).length]));
  const list = W.filter ? issues.filter((i) => i.category === W.filter) : issues;
  const scrollTop = body.scrollTop;
  body.innerHTML = `
    <div class="w-sum">${wRing(d.score, d.busy)}
      <div class="grow col" style="gap:2px"><b>Điểm tổng</b>
        <span class="small muted">${d.busy ? 'Đang kiểm tra…' : !d.text ? 'Ô soạn tin đang trống.' : d.score == null ? 'Chưa kiểm tra' : issues.length ? `${issues.length} gợi ý` : 'Không thấy lỗi nào. 👍'}</span></div></div>
    <div class="row wrap" style="gap:8px">
      <button class="btn gold sm" id="w-check" ${d.busy || !d.text ? 'disabled' : ''}>Kiểm tra lại</button>
      <button class="chip-toggle ${d.autoCheck ? 'on' : ''}" id="w-auto">Kiểm tra khi gõ</button>
    </div>
    <div id="w-choice"></div>
    <div class="w-cats">${W_CAT.map(([k, label, color]) => `<button class="w-cat ${W.filter === k ? 'on' : ''}" data-cat="${k}" style="--c:${color}">
      <b style="color:${counts[k] ? color : 'var(--text-3)'}">${counts[k]}</b><span>${label}</span></button>`).join('')}</div>
    ${d.tone ? `<div class="small" style="color:${d.toneFits ? 'var(--text-2)' : '#a78bfa'}">Giọng điệu: <span data-noi18n>${esc(d.tone)}</span>${d.toneFits ? '' : ' — chưa hợp mục tiêu của bạn'}</div>` : ''}
    <div class="w-tabs">${[['list', 'Gợi ý'], ['goals', 'Mục tiêu'], ['write', 'Viết']].map(([k, l]) => `<button data-wt="${k}" class="${W.tab === k ? 'on' : ''}">${l}</button>`).join('')}</div>
    <div id="w-pane" class="col" style="gap:8px"></div>`;
  body.scrollTop = scrollTop;
  choiceBar($('#w-choice'), d.choice || S.settings.ai.writing, d.providers || S.providers, (c) => { d.choice = c; invoke('assist_save_choice', { choice: c, panel: false }); });
  $('#w-check').addEventListener('click', () => invoke('writing_action', { account: W.account, action: 'recheck', original: null }).catch((e) => toast(errText(e), true)));
  $('#w-auto').addEventListener('click', async () => {
    d.autoCheck = !d.autoCheck;
    S.settings.ai.auto_check = d.autoCheck;
    $('#w-auto').classList.toggle('on', d.autoCheck);
    try { await invoke('set_auto_check', { on: d.autoCheck }); } catch (e) { toast(errText(e), true); }
  });
  body.querySelectorAll('[data-cat]').forEach((b) => b.addEventListener('click', () => { W.filter = W.filter === b.dataset.cat ? null : b.dataset.cat; renderWritingBody(); }));
  body.querySelectorAll('[data-wt]').forEach((b) => b.addEventListener('click', () => { W.tab = b.dataset.wt; renderWritingBody(); }));
  const pane = $('#w-pane');
  if (W.tab === 'goals') return renderWritingGoals(pane, d);
  if (W.tab === 'write') return renderWritingWrite(pane, d);
  pane.innerHTML = list.map((i, n) => {
    const fresh = !W.known.has(i.original);
    const open = W.open === i.original;
    return `<div class="w-item ${fresh ? 'fresh' : ''} ${open ? 'open' : ''}" data-o="${esc(i.original)}" style="--c:${W_COLOR[i.category] || W_COLOR.correctness};--i:${n}">
      <div class="row" style="gap:8px"><span class="w-dot"></span><span class="grow" data-noi18n><del>${esc(i.original)}</del> → <ins>${i.replacement ? esc(i.replacement) : '(xóa)'}</ins></span></div>
      <div class="small muted" ${i.title ? 'data-noi18n' : ''}>${esc(i.title || '')}</div>
      <div class="w-more"><div class="col" style="gap:6px">${i.explanation ? `<div class="small" data-noi18n>${esc(i.explanation)}</div>` : ''}
        <div class="row wrap" style="gap:6px"><button class="btn sm primary" data-a="accept">Chấp nhận</button><button class="btn sm" data-a="dismiss">Bỏ qua</button><button class="btn sm ghost" data-a="learn">Thêm vào từ điển</button></div></div></div>
    </div>`;
  }).join('') || (d.busy ? '' : `<div class="small faint">${d.text ? 'Không có gợi ý nào.' : ''}</div>`);
  issues.forEach((i) => W.known.add(i.original));
  pane.querySelectorAll('.w-item').forEach((card) => {
    card.addEventListener('click', (e) => {
      const btn = e.target.closest('[data-a]');
      const original = card.dataset.o;
      if (btn) {
        e.stopPropagation();
        card.classList.add('leaving');
        invoke('writing_action', { account: W.account, action: btn.dataset.a, original }).catch((err) => toast(errText(err), true));
        return;
      }
      // Open the card and light up the same words in the message box.
      W.open = W.open === original ? null : original;
      pane.querySelectorAll('.w-item').forEach((c) => c.classList.toggle('open', c.dataset.o === W.open));
      if (W.open) invoke('writing_action', { account: W.account, action: 'focus', original }).catch(() => {});
    });
  });
}

function renderWritingGoals(pane, d) {
  const g = Object.assign({ formality: 'neutral', audience: 'general', intent: 'chat', variant: 'auto' }, d.goals || {});
  const dict = d.dictionary || [];
  pane.innerHTML = `
    <div class="small muted">Trợ lý kiểm tra bản nháp theo các mục tiêu này.</div>
    <div class="w-goals">${Object.keys(W_GOALS).map((k) => `<span class="small muted">${W_GOALS[k][0]}</span>
      <select data-g="${k}">${W_GOALS[k][1].map(([v, l]) => `<option value="${v}" ${g[k] === v ? 'selected' : ''}>${l}</option>`).join('')}</select>`).join('')}</div>
    <b class="small">Từ điển của bạn</b>
    <div class="small muted">Những từ trợ lý không bao giờ đánh dấu (tên riêng, tiếng lóng…).</div>
    <div class="row"><input type="text" id="w-word" class="grow" placeholder="Thêm từ…"><button class="btn sm" id="w-add">Thêm</button></div>
    <div class="row wrap" style="gap:6px">${dict.map((w, n) => `<span class="w-word" data-noi18n>${esc(w)}<button data-del="${n}" title="Xóa">×</button></span>`).join('') || '<span class="small faint">Chưa có từ nào.</span>'}</div>`;
  const save = () => invoke('assist_save_goals', { goals: g, dictionary: dict }).catch((e) => toast(errText(e), true));
  pane.querySelectorAll('[data-g]').forEach((s) => s.addEventListener('change', () => { g[s.dataset.g] = s.value; d.goals = g; save(); }));
  const add = () => {
    const w = $('#w-word').value.trim();
    if (!w) return;
    if (!dict.some((x) => x.toLowerCase() === w.toLowerCase())) dict.push(w);
    d.dictionary = dict; save(); renderWritingGoals(pane, d);
  };
  $('#w-add').addEventListener('click', add);
  $('#w-word').addEventListener('keydown', (e) => { if (e.key === 'Enter') add(); });
  pane.querySelectorAll('[data-del]').forEach((b) => b.addEventListener('click', () => { dict.splice(+b.dataset.del, 1); d.dictionary = dict; save(); renderWritingGoals(pane, d); }));
}

function renderWritingWrite(pane, d) {
  const t = W.text;
  pane.innerHTML = `
    <div class="small muted">Viết lại bản nháp trong ô soạn tin, dùng skill, hoặc bảo AI viết điều bạn muốn.</div>
    <div class="row wrap" style="gap:6px">${W_WRITE.map(([l], n) => `<button class="chip" data-w="${n}">${l}</button>`).join('')}
      ${(d.skills || []).map((k) => `<button class="chip" data-skill="${esc(k.id)}">${esc(k.name)}</button>`).join('')}</div>
    <div class="row"><textarea id="w-req" rows="2" class="grow" placeholder="Hoặc bảo AI viết gì, vd: trả lời lịch sự rằng mình sẽ đến muộn 15 phút…"></textarea><button class="btn primary sm" id="w-go">Viết</button></div>
    ${W.busy ? '<div class="row muted small"><div class="spinner"></div>AI đang viết…</div>' : ''}
    ${t ? `<div class="result w-result"><div style="white-space:pre-wrap" data-noi18n>${esc(t)}</div>
      <div class="row" style="margin-top:8px;gap:6px"><button class="btn sm primary" id="w-use">Thay vào ô soạn tin</button><button class="btn sm" id="w-copy">Sao chép</button></div></div>` : ''}`;
  const run = async (action, skillId, instruction) => {
    if (W.busy) return;
    W.busy = true; W.text = null; renderWritingWrite(pane, d);
    try {
      const v = await invoke('assist_run', { action, text: d.text || ' ', skillId, instruction, choice: d.choice || S.settings.ai.writing });
      W.text = v.text;
    } catch (e) { toast(errText(e), true); }
    W.busy = false;
    if (W.tab === 'write' && $('#w-pane')) renderWritingWrite($('#w-pane'), d);
  };
  pane.querySelectorAll('[data-w]').forEach((b) => b.addEventListener('click', () => run('custom', null, W_WRITE[+b.dataset.w][1])));
  pane.querySelectorAll('[data-skill]').forEach((b) => b.addEventListener('click', () => run('skill', b.dataset.skill, null)));
  const go = () => { const r = $('#w-req').value.trim(); if (r) run('custom', null, r); };
  $('#w-go').addEventListener('click', go);
  $('#w-req').addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); go(); } });
  const use = $('#w-use');
  if (use) use.addEventListener('click', () => invoke('writing_apply', { account: W.account, text: t }).then(() => toast('Đã thay vào ô soạn tin')).catch((e) => toast(errText(e), true)));
  const cp = $('#w-copy');
  if (cp) cp.addEventListener('click', () => { navigator.clipboard.writeText(t); toast('Đã sao chép'); });
}
