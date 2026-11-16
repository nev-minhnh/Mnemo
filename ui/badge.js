// Unread count on the taskbar button (Windows overlay icon) and on the tray icon. Windows has
// no numeric badge for desktop apps, so both are drawn here and handed over as small images.
// On a taskbar with small icons Windows shrinks the overlay, which then reads as a red dot.
'use strict';

const Badge = (() => {
  let last = '';
  let logo = null;

  function loadLogo() {
    if (logo) return Promise.resolve(logo);
    return new Promise((res) => {
      const img = new Image();
      img.onload = () => res((logo = img));
      img.onerror = () => res(null);
      img.src = 'logo.png';
    });
  }

  const label = (n) => (n > 99 ? '99+' : String(n));

  // A red badge with a white ring, filling a size² square at (x, y).
  function draw(g, x, y, size, n) {
    const r = size / 2;
    g.beginPath();
    g.arc(x + r, y + r, r - 0.5, 0, Math.PI * 2);
    g.fillStyle = '#ffffff';
    g.fill();
    g.beginPath();
    g.arc(x + r, y + r, r - Math.max(1.5, size * 0.07), 0, Math.PI * 2);
    g.fillStyle = '#d92d3a';
    g.fill();
    const s = label(n);
    const k = s.length === 1 ? 0.66 : s.length === 2 ? 0.54 : 0.4;
    g.fillStyle = '#ffffff';
    g.font = `700 ${Math.round(size * k)}px "Segoe UI Variable Display", "Segoe UI", Arial, sans-serif`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillText(s, x + r, y + r + size * 0.04);
  }

  async function png(c) {
    const blob = await new Promise((res) => c.toBlob(res, 'image/png'));
    return blob ? Array.from(new Uint8Array(await blob.arrayBuffer())) : [];
  }

  async function update(total, on) {
    const n = on ? Math.max(0, total | 0) : 0;
    const key = String(n);
    if (key === last) return;
    last = key;
    try {
      if (n <= 0) {
        await invoke('set_unread_badge', { count: total | 0, overlay: null, tray: null });
        return;
      }
      const o = document.createElement('canvas');
      o.width = o.height = 32;
      draw(o.getContext('2d'), 0, 0, 32, n);
      // Tray: the logo with the badge in its top-right corner.
      const t = document.createElement('canvas');
      t.width = t.height = 32;
      const g = t.getContext('2d');
      const img = await loadLogo();
      if (img) g.drawImage(img, 0, 0, 32, 32);
      draw(g, 13, 0, 19, n);
      await invoke('set_unread_badge', { count: n, overlay: await png(o), tray: await png(t) });
    } catch (e) {
      last = '';
    }
  }

  return { update };
})();
