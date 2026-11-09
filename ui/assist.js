// The writing popup: proofreading, skills, free requests, then put the result back
// into the app's draft box.
'use strict';

const A = { sig: '', loadedAt: 0, state: null, choice: null, action: 'check', skill: null, busy: false, original: '', issues: [] };
const TYPE_LABEL = { spelling: 'Chính tả', grammar: 'Ngữ pháp', syntax: 'Cú pháp', punctuation: 'Dấu câu', word_choice: 'Dùng từ', style: 'Văn phong' };

function setStatus(html) { $('#status').innerHTML = html || ''; }

function renderActions() {
  const box = $('#actions');
  const skills = (A.state && A.state.skills) || [];
  box.innerHTML = `<button class="chip ${A.action === 'check' ? 'active' : ''}" data-a="check">✓ Sửa lỗi</button>`
    + skills.map((k) => `<button class="chip ${A.action === 'skill' && A.skill === k.id ? 'active' : ''}" data-skill="${esc(k.id)}" title="${esc(k.instruction)}">${esc(k.name)}</button>`).join('');
  box.querySelector('[data-a="check"]').addEventListener('click', () => { A.action = 'check'; A.skill = null; renderActions(); run(); });
  box.querySelectorAll('[data-skill]').forEach((b) => b.addEventListener('click', () => {
    A.action = 'skill';
    A.skill = b.dataset.skill;
    renderActions();
    run();
  }));
}

function showCheck(v) {
  A.issues = (v.issues || []).filter((i) => i && i.original !== undefined);
  const out = $('#out');
  if (!A.issues.length) {
    out.innerHTML = '<div class="notice ok">Không thấy lỗi nào. 👍</div>';
    return;
  }
  out.innerHTML = `<div class="row"><b class="grow">${A.issues.length} gợi ý</b><button class="btn sm" id="fix-all">Sửa tất cả</button></div>`
    + A.issues.map((i, n) => `<div class="issue" data-n="${n}">
      <div class="row"><span class="badge accent">${esc(TYPE_LABEL[i.type] || i.type || 'Gợi ý')}</span><span class="grow"></span><button class="btn sm" data-fix="${n}">Sửa</button></div>
      <div data-noi18n><del>${esc(i.original)}</del> → <ins>${esc(i.suggestion)}</ins></div>
      ${i.explanation ? `<div class="small muted" data-noi18n>${esc(i.explanation)}</div>` : ''}
    </div>`).join('');
  $('#fix-all').addEventListener('click', () => {
    $('#draft').value = v.corrected || $('#draft').value;
    out.querySelectorAll('.issue').forEach((n) => n.classList.add('done'));
  });
  out.querySelectorAll('[data-fix]').forEach((b) => b.addEventListener('click', () => {
    const i = A.issues[+b.dataset.fix];
    const d = $('#draft');
    if (i.original && d.value.includes(i.original)) d.value = d.value.replace(i.original, i.suggestion || '');
    b.closest('.issue').classList.add('done');
  }));
}

function showText(v) {
  $('#out').innerHTML = `<div class="result"><div class="row"><b class="grow"${v.title ? ' data-noi18n' : ''}>${esc(v.title || 'Kết quả')}</b><button class="btn sm" id="use">Dùng bản này</button></div>
    <div style="white-space:pre-wrap;margin-top:6px" data-noi18n>${esc(v.text)}</div></div>`;
  $('#use').addEventListener('click', () => { $('#draft').value = v.text; });
}

async function run(instruction) {
  if (A.busy) return;
  const text = $('#draft').value.trim() ? $('#draft').value : A.original;
  if (!text.trim()) { setStatus('<div class="notice">Ô soạn tin đang trống.</div>'); return; }
  A.busy = true;
  $('#out').innerHTML = '';
  setStatus('<div class="row muted small"><div class="spinner"></div>Đang hỏi AI…</div>');
  try {
    const action = instruction ? 'custom' : A.action;
    const v = await invoke('assist_run', {
      action, text: action === 'check' ? A.original : text,
      skillId: A.action === 'skill' ? A.skill : null,
      instruction: instruction || null, choice: A.choice,
    });
    setStatus('');
    if (v.kind === 'check') showCheck(v); else showText(v);
  } catch (e) {
    setStatus(`<div class="notice danger">${esc(errText(e))}</div>`);
  }
  A.busy = false;
}

async function load() {
  const st = await invoke('assist_state');
  // The window's first load and the "open" event can arrive together: handle it once.
  const sig = (st.text || '') + '|' + (st.account || '');
  if (sig === A.sig && Date.now() - A.loadedAt < 2000) return;
  A.sig = sig;
  A.loadedAt = Date.now();
  A.state = st;
  applyTheme(st.theme);
  setLang(st.language);
  A.choice = Object.assign({}, A.state.choice);
  A.original = A.state.text || '';
  A.action = 'check';
  A.skill = null;
  $('#draft').value = A.original;
  $('#req').value = '';
  $('#where').textContent = A.state.account ? `${A.state.account} · ${A.state.service}` : '';
  choiceBar($('#choice'), A.choice, A.state.providers, (c) => invoke('assist_save_choice', { choice: c, panel: false }));
  renderActions();
  $('#out').innerHTML = '';
  setStatus('');
  if (A.state.cached) showCheck(A.state.cached);
  else run();
}

$('#close').innerHTML = ICON.close;
$('#close').addEventListener('click', () => invoke('assist_close'));
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('assist_close'); });
$('#send').addEventListener('click', () => { const r = $('#req').value.trim(); if (r) run(r); });
$('#req').addEventListener('keydown', (e) => { if (e.key === 'Enter') { const r = e.target.value.trim(); if (r) run(r); } });
$('#apply').addEventListener('click', async () => {
  try { await invoke('assist_apply', { text: $('#draft').value }); } catch (e) { setStatus(`<div class="notice danger">${esc(errText(e))}</div>`); }
});
$('#copy').addEventListener('click', () => { navigator.clipboard.writeText($('#draft').value); toast('Đã sao chép'); });
listen('assist-open', () => load().catch((e) => setStatus(`<div class="notice danger">${esc(errText(e))}</div>`)));
load().catch(() => {});
