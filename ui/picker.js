// The Add account popover: pick one of the apps, name the account, add it.
'use strict';

let P = null;
let busy = false;

const PARTNER = { instagram: 'messenger', threads: 'instagram' };

function close() { invoke('picker_close').catch(() => {}); }

function stepApps() {
  $('#body').innerHTML = `
    <div class="head"><h2>Thêm tài khoản</h2><button class="btn icon ghost sm" id="x" title="Đóng (Esc)">${ICON.close}</button></div>
    <div class="grid">${P.services.map((s) => `<button class="tile" data-svc="${esc(s.id)}">${svcIcon(s.id, 42)}<span>${esc(s.name)}</span></button>`).join('')}</div>
    <div class="hint">Mỗi tài khoản có hồ sơ trình duyệt riêng, nên bạn thêm được nhiều tài khoản của cùng một ứng dụng.</div>`;
  $('#x').addEventListener('click', close);
  $$('[data-svc]').forEach((b) => b.addEventListener('click', () => stepName(b.dataset.svc)));
  const first = $('[data-svc]');
  if (first) first.focus();
}

function stepName(service) {
  const name = SERVICE_NAME[service] || service;
  const partner = PARTNER[service];
  // Accounts of the partner app whose sign-in this one can continue with.
  const candidates = partner ? P.accounts.filter((a) => a.service === partner && !a.disabled) : [];
  $('#body').innerHTML = `
    <div class="head"><button class="btn icon ghost sm" id="back" title="Quay lại">‹</button>${svcIcon(service, 28)}<h2>Thêm ${esc(name)}</h2><button class="btn icon ghost sm" id="x" title="Đóng (Esc)">${ICON.close}</button></div>
    <div class="step2">
      <label class="col" style="gap:4px"><span class="small muted">Tên hiển thị (để phân biệt nhiều tài khoản)</span>
        <input type="text" id="label" placeholder="${esc(name)}" maxlength="60"></label>
      ${candidates.length ? `<div class="col" style="gap:6px;min-height:0">
        <span class="small muted">Đăng nhập</span>
        <div class="opts">
          ${candidates.map((a, i) => `<label class="opt"><input type="radio" name="share" value="${esc(a.id)}" ${i === 0 ? 'checked' : ''}>
            ${svcIcon(a.service, 20)}<span class="grow"><b data-noi18n>${esc(a.label)}</b> <span class="small muted">· dùng luôn đăng nhập ${esc(SERVICE_NAME[a.service])}</span></span></label>`).join('')}
          <label class="opt"><input type="radio" name="share" value="" ${candidates.length ? '' : 'checked'}><span class="grow">Đăng nhập tài khoản khác</span></label>
        </div>
        <span class="hint">${service === 'instagram' ? 'Instagram sẽ có nút "Tiếp tục với tư cách…" tài khoản Facebook này.' : 'Threads sẽ có nút "Tiếp tục bằng Instagram" với tài khoản này.'}</span>
      </div>` : ''}
      <div class="foot"><button class="btn" id="cancel">Hủy</button><button class="btn primary" id="ok">Thêm và đăng nhập</button></div>
    </div>`;
  $('#x').addEventListener('click', close);
  $('#cancel').addEventListener('click', close);
  $('#back').addEventListener('click', stepApps);
  const input = $('#label');
  input.focus();
  const ok = async () => {
    if (busy) return;
    busy = true;
    const btn = $('#ok');
    btn.disabled = true;
    btn.insertAdjacentHTML('afterbegin', '<span class="spinner"></span>');
    const share = document.querySelector('input[name="share"]:checked');
    try {
      await invoke('add_account', { service, label: input.value, shareWith: share && share.value ? share.value : null });
      close();
    } catch (e) {
      btn.disabled = false;
      const sp = btn.querySelector('.spinner');
      if (sp) sp.remove();
      const err = el(`<div class="notice danger small"></div>`);
      err.textContent = errText(e);
      btn.closest('.foot').before(err);
    }
    busy = false;
  };
  $('#ok').addEventListener('click', ok);
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') ok(); });
}

async function open(payload) {
  P = await invoke('picker_state');
  payload = payload || P.place;
  window.SVC_ICONS = P.icons || {};
  applyTheme(P.theme);
  setLang(P.language);
  const side = (payload && payload.side) || 'left';
  const pop = $('#pop');
  pop.className = 'pop side-' + side;
  const a = $('#arrow');
  const at = Math.max(14, ((payload && payload.arrow) || 40) - 10 - 7);
  a.style.top = a.style.left = '';
  if (side === 'top' || side === 'bottom') a.style.left = at + 'px'; else a.style.top = at + 'px';
  busy = false;
  stepApps();
}

document.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
listen('picker-open', (e) => open(e.payload).catch(() => {}));
// First load: the window may have opened before this page could listen.
open(null).catch(() => {});
