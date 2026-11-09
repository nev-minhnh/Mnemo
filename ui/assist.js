// The writing popup, working like Grammarly for one chat draft (never the conversation):
// suggestions in four categories with a score and the tone, writing with AI (rewrites,
// skills, free requests), and writing goals plus a personal dictionary.
'use strict';

const A = {
  sig: '', loadedAt: 0, state: null, choice: null, tab: 'check', original: '', busy: false,
  result: null, filter: null, dismissed: new Set(), text: null,
};
const CAT = {
  correctness: 'Đúng',
  clarity: 'Rõ ràng',
  engagement: 'Hấp dẫn',
  delivery: 'Giọng điệu',
};
const GOALS = {
  formality: [['casual', 'Thân mật'], ['neutral', 'Trung tính'], ['formal', 'Trang trọng']],
  audience: [['general', 'Chung'], ['friends', 'Bạn bè'], ['family', 'Gia đình'], ['colleagues', 'Đồng nghiệp'], ['customers', 'Khách hàng'], ['boss', 'Cấp trên']],
  intent: [['chat', 'Trò chuyện'], ['inform', 'Thông báo'], ['request', 'Nhờ / đề nghị'], ['persuade', 'Thuyết phục'], ['apologise', 'Xin lỗi'], ['thank', 'Cảm ơn']],
  variant: [['auto', 'Theo bản nháp'], ['vi', 'Tiếng Việt'], ['en-US', 'Tiếng Anh (Mỹ)'], ['en-GB', 'Tiếng Anh (Anh)']],
};
const GOAL_LABEL = { formality: 'Mức trang trọng', audience: 'Người nhận', intent: 'Mục đích', variant: 'Ngôn ngữ' };
const WRITE = [
  ['Trang trọng hơn', 'Rewrite it in a more formal, professional tone.'],
  ['Thân thiện hơn', 'Rewrite it in a warmer, friendlier tone.'],
  ['Ngắn gọn hơn', 'Make it more concise without losing meaning.'],
  ['Mở rộng', 'Expand it with a little more detail, still as a chat message.'],
  ['Đơn giản hơn', 'Rewrite it in simpler words and shorter sentences.'],
  ['Hay hơn', 'Improve its clarity, flow and word choice.'],
];

function setStatus(html) { $('#status').innerHTML = html || ''; }

function issuesShown() {
  const r = A.result;
  if (!r || r.kind !== 'check') return [];
  return (r.issues || []).filter((i) => !A.dismissed.has(i.original));
}

function scoreRing(score) {
  const v = Math.max(0, Math.min(100, Math.round(score)));
  const color = v >= 90 ? 'var(--ok)' : v >= 70 ? 'var(--gold-2)' : 'var(--danger)';
  const c = 2 * Math.PI * 22;
  return `<div class="ring" title="Điểm viết"><svg viewBox="0 0 52 52"><circle cx="26" cy="26" r="22" fill="none" stroke="var(--surface-3)" stroke-width="5"/>
    <circle cx="26" cy="26" r="22" fill="none" stroke="${color}" stroke-width="5" stroke-linecap="round" stroke-dasharray="${c}" stroke-dashoffset="${c * (1 - v / 100)}"/></svg><b>${v}</b></div>`;
}

// ---------------------------------------------------------------- tabs

function render() {
  $$('#tabs [data-tab]').forEach((b) => b.classList.toggle('active', b.dataset.tab === A.tab));
  if (A.tab === 'check') renderCheck();
  else if (A.tab === 'write') renderWrite();
  else renderGoals();
}

function renderCheck() {
  const pane = $('#pane');
  const r = A.result;
  if (!r || r.kind !== 'check') {
    pane.innerHTML = A.busy ? '' : `<button class="btn primary" id="run-check">✓ Kiểm tra bản nháp</button>`;
    const b = $('#run-check');
    if (b) b.addEventListener('click', () => runCheck());
    return;
  }
  const shown = issuesShown();
  const counts = {};
  shown.forEach((i) => { counts[i.category] = (counts[i.category] || 0) + 1; });
  const list = A.filter ? shown.filter((i) => i.category === A.filter) : shown;
  pane.innerHTML = `
    <div class="summary">${scoreRing(r.score ?? 100)}
      <div class="grow col" style="gap:2px">
        <b>${shown.length ? `${shown.length} gợi ý` : 'Không thấy lỗi nào. 👍'}</b>
        ${r.tone ? `<span class="small muted">Giọng điệu: <b data-noi18n>${esc(r.tone)}</b> ${r.tone_fits === false ? '<span class="badge warn">chưa hợp mục tiêu</span>' : '<span class="badge ok">hợp mục tiêu</span>'}</span>` : ''}
      </div>
      ${shown.length ? '<button class="btn sm" id="fix-all">Sửa tất cả</button>' : ''}
    </div>
    ${shown.length ? `<div class="chips">
      <button class="chip ${A.filter ? '' : 'active'}" data-f="">Tất cả · ${shown.length}</button>
      ${Object.keys(CAT).filter((c) => counts[c]).map((c) => `<button class="chip c-${c} ${A.filter === c ? 'active' : ''}" data-f="${c}"><span class="dot"></span>${CAT[c]} · ${counts[c]}</button>`).join('')}
    </div>` : ''}
    ${list.map((i) => `<div class="issue c-${esc(i.category)}" data-o="${esc(i.original)}">
      <div class="row"><span class="dot"></span><span class="title grow"${i.title ? ' data-noi18n' : ''}>${esc(i.title || CAT[i.category] || 'Gợi ý')}</span></div>
      <div data-noi18n><del>${esc(i.original)}</del> → <ins>${i.replacement ? esc(i.replacement) : '<i>(xóa)</i>'}</ins></div>
      ${i.explanation ? `<div class="small muted" data-noi18n>${esc(i.explanation)}</div>` : ''}
      <div class="acts"><button class="btn sm primary" data-act="accept">Chấp nhận</button><button class="btn sm" data-act="dismiss">Bỏ qua</button><button class="btn sm ghost" data-act="learn">Thêm vào từ điển</button></div>
    </div>`).join('')}`;
  pane.querySelectorAll('[data-f]').forEach((b) => b.addEventListener('click', () => { A.filter = b.dataset.f || null; renderCheck(); }));
  const fa = $('#fix-all');
  if (fa) fa.addEventListener('click', () => {
    let d = $('#draft').value;
    shown.forEach((i) => { if (d.includes(i.original)) d = d.replace(i.original, i.replacement || ''); });
    $('#draft').value = d;
    shown.forEach((i) => A.dismissed.add(i.original));
    renderCheck();
  });
  pane.querySelectorAll('.issue').forEach((card) => card.querySelectorAll('[data-act]').forEach((b) => b.addEventListener('click', async () => {
    const i = shown.find((x) => x.original === card.dataset.o);
    if (!i) return;
    if (b.dataset.act === 'accept') {
      const d = $('#draft');
      if (d.value.includes(i.original)) d.value = d.value.replace(i.original, i.replacement || '');
    }
    if (b.dataset.act === 'learn') {
      try { await invoke('assist_learn', { word: i.original }); A.state.dictionary.push(i.original); toast('Đã thêm vào từ điển'); } catch (e) { toast(errText(e), true); }
    }
    A.dismissed.add(i.original);
    renderCheck();
  })));
}

function renderWrite() {
  const skills = (A.state && A.state.skills) || [];
  const t = A.text;
  $('#pane').innerHTML = `
    <div class="small muted">Viết lại bản nháp, dùng skill, hoặc bảo AI viết điều bạn muốn.</div>
    <div class="chips">${WRITE.map(([l], n) => `<button class="chip" data-w="${n}">${l}</button>`).join('')}
      ${skills.map((k) => `<button class="chip" data-skill="${esc(k.id)}" title="${esc(k.instruction)}">${esc(k.name)}</button>`).join('')}</div>
    <div class="row"><textarea id="req" rows="2" class="grow" placeholder="Hoặc bảo AI viết gì, vd: trả lời lịch sự rằng mình sẽ đến muộn 15 phút…"></textarea><button class="btn primary sm" id="send">Viết</button></div>
    ${t ? `<div class="result"><div class="row"><b class="grow"${t.title ? ' data-noi18n' : ''}>${esc(t.title || 'Kết quả')}</b><button class="btn sm primary" id="use">Dùng bản này</button></div>
      <div style="white-space:pre-wrap;margin-top:6px" data-noi18n>${esc(t.text)}</div></div>` : ''}`;
  $$('[data-w]').forEach((b) => b.addEventListener('click', () => runText('custom', null, WRITE[+b.dataset.w][1], WRITE[+b.dataset.w][0])));
  $$('[data-skill]').forEach((b) => b.addEventListener('click', () => runText('skill', b.dataset.skill, null)));
  const go = () => { const r = $('#req').value.trim(); if (r) runText('custom', null, r); };
  $('#send').addEventListener('click', go);
  $('#req').addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); go(); } });
  const use = $('#use');
  if (use) use.addEventListener('click', () => { $('#draft').value = t.text; });
}

function renderGoals() {
  const g = Object.assign({}, A.state.goals);
  const dict = A.state.dictionary || [];
  $('#pane').innerHTML = `
    <div class="small muted">Trợ lý kiểm tra bản nháp theo các mục tiêu này.</div>
    <div class="goals">${Object.keys(GOALS).map((k) => `<span class="small muted">${GOAL_LABEL[k]}</span>
      <select data-g="${k}">${GOALS[k].map(([v, l]) => `<option value="${v}" ${g[k] === v ? 'selected' : ''}>${l}</option>`).join('')}</select>`).join('')}</div>
    <div class="col" style="gap:6px"><b class="small">Từ điển của bạn</b>
      <div class="small muted">Những từ trợ lý không bao giờ đánh dấu (tên riêng, tiếng lóng…).</div>
      <div class="row"><input type="text" id="new-word" class="grow" placeholder="Thêm từ…"><button class="btn sm" id="add-word">Thêm</button></div>
      <div class="words">${dict.map((w, n) => `<span class="word" data-noi18n>${esc(w)}<button data-del="${n}" title="Xóa">×</button></span>`).join('') || '<span class="small faint">Chưa có từ nào.</span>'}</div>
    </div>`;
  const save = async () => {
    try { await invoke('assist_save_goals', { goals: A.state.goals, dictionary: A.state.dictionary }); } catch (e) { toast(errText(e), true); }
  };
  $$('[data-g]').forEach((s) => s.addEventListener('change', () => { A.state.goals[s.dataset.g] = s.value; A.result = null; save(); }));
  const add = () => {
    const w = $('#new-word').value.trim();
    if (!w) return;
    if (!A.state.dictionary.some((x) => x.toLowerCase() === w.toLowerCase())) A.state.dictionary.push(w);
    save(); renderGoals();
  };
  $('#add-word').addEventListener('click', add);
  $('#new-word').addEventListener('keydown', (e) => { if (e.key === 'Enter') add(); });
  $$('[data-del]').forEach((b) => b.addEventListener('click', () => { A.state.dictionary.splice(+b.dataset.del, 1); save(); renderGoals(); }));
}

// ---------------------------------------------------------------- running

async function runCheck() {
  if (A.busy) return;
  const text = A.original;
  if (!text.trim()) { setStatus('<div class="notice">Ô soạn tin đang trống.</div>'); return; }
  A.busy = true;
  A.dismissed.clear();
  A.filter = null;
  render();
  setStatus('<div class="row muted small"><div class="spinner"></div>Đang kiểm tra…</div>');
  try {
    A.result = await invoke('assist_run', { action: 'check', text, skillId: null, instruction: null, choice: A.choice });
    setStatus('');
  } catch (e) {
    setStatus(`<div class="notice danger">${esc(errText(e))}</div>`);
  }
  A.busy = false;
  if (A.tab === 'check') render();
}

async function runText(action, skillId, instruction, title) {
  if (A.busy) return;
  const text = $('#draft').value.trim() ? $('#draft').value : A.original;
  if (!text.trim() && action !== 'custom') { setStatus('<div class="notice">Ô soạn tin đang trống.</div>'); return; }
  A.busy = true;
  A.text = null;
  render();
  setStatus('<div class="row muted small"><div class="spinner"></div>AI đang viết…</div>');
  try {
    const v = await invoke('assist_run', { action, text: text || ' ', skillId, instruction, choice: A.choice });
    A.text = { text: v.text, title: title || v.title };
    setStatus('');
  } catch (e) {
    setStatus(`<div class="notice danger">${esc(errText(e))}</div>`);
  }
  A.busy = false;
  if (A.tab === 'write') render();
}

async function load() {
  const st = await invoke('assist_state');
  // The window's first load and the "open" event can arrive together: handle it once.
  const sig = (st.text || '') + '|' + (st.account || '');
  if (sig === A.sig && Date.now() - A.loadedAt < 2000) return;
  A.sig = sig;
  A.loadedAt = Date.now();
  A.state = st;
  A.state.goals = A.state.goals || { formality: 'neutral', audience: 'general', intent: 'chat', variant: 'auto' };
  A.state.dictionary = A.state.dictionary || [];
  applyTheme(st.theme);
  setLang(st.language);
  A.choice = Object.assign({}, A.state.choice);
  A.original = A.state.text || '';
  A.result = st.cached || null;
  A.text = null;
  A.dismissed.clear();
  A.filter = null;
  A.tab = 'check';
  $('#draft').value = A.original;
  $('#where').textContent = A.state.account ? `${A.state.account} · ${A.state.service}` : '';
  choiceBar($('#choice'), A.choice, A.state.providers, (c) => invoke('assist_save_choice', { choice: c, panel: false }));
  setStatus('');
  render();
  if (!A.result) runCheck();
}

$('#close').innerHTML = ICON.close;
$('#close').addEventListener('click', () => invoke('assist_close'));
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('assist_close'); });
$$('#tabs [data-tab]').forEach((b) => b.addEventListener('click', () => { A.tab = b.dataset.tab; render(); if (A.tab === 'check' && !A.result) runCheck(); }));
$('#apply').addEventListener('click', async () => {
  try { await invoke('assist_apply', { text: $('#draft').value }); } catch (e) { setStatus(`<div class="notice danger">${esc(errText(e))}</div>`); }
});
$('#copy').addEventListener('click', () => { navigator.clipboard.writeText($('#draft').value); toast('Đã sao chép'); });
listen('assist-open', () => load().catch((e) => setStatus(`<div class="notice danger">${esc(errText(e))}</div>`)));
load().catch(() => {});
