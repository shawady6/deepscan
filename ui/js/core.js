/* DeepScan UI core: state, API bridge, router, small components. Plain scripts (no modules) so it works from any WebView. */
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const sleep = ms => new Promise(r => setTimeout(r, ms));
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

const ICONS = {
  overview: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  hardware: '<rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9.5" y="9.5" width="5" height="5"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
  storage: '<ellipse cx="12" cy="6" rx="8" ry="3"/><path d="M4 6v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6"/>',
  software: '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M4 9h16M9 9v11"/>',
  network: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"/>',
  security: '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/>',
  reports: '<path d="M6 3h9l4 4v14H6z"/><path d="M14 3v5h5M9 13h7M9 17h7"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>',
  check: '<path d="M6 10.5l2.8 2.8L14 8"/>',
};
const icon = (name, size = 18) => `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name] || ''}</svg>`;

const NAV = [
  ['overview', 'Overview'], ['hardware', 'Hardware'], ['storage', 'Storage'], ['software', 'Software'],
  ['network', 'Network'], ['security', 'Security'], ['reports', 'Reports'],
];
const STEPS = [
  ['hardware', 'Hardware and sensors'], ['storage', 'Storage and health'], ['network', 'Network and latency'],
  ['security', 'Security checks'], ['software', 'Installed software and startup'],
];

/* ---------- formatting ---------- */
const fmt = {
  gb: v => v == null ? '-' : (v >= 100 ? Math.round(v) : v) + ' GB',
  mb: v => v == null ? '-' : (v >= 1024 ? (v / 1024).toFixed(1) + ' GB' : Math.round(v) + ' MB'),
  uptime: s => { s = Math.max(0, s | 0); const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), m = Math.floor(s % 3600 / 60); return d ? `${d} d ${String(h).padStart(2, '0')} h` : h ? `${h} h ${String(m).padStart(2, '0')} m` : `${m} min`; },
  hours: h => h == null ? '-' : h.toLocaleString('en-US') + ' h',
  dash: v => (v === null || v === undefined || v === '') ? '-' : v,
  time: d => d ? d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '',
};

/* ---------- small components ---------- */
const STATUS = { ok: ['ok', 'OK'], warn: ['warn', 'Warning'], bad: ['bad', 'Critical'], unknown: ['info', 'Unknown'] };
const badge = (kind, text, fixed) => `<span class="badge ${kind}${fixed ? ' fixed' : ''}">${esc(text)}</span>`;
const statusBadge = (s, fixed = true) => { const [k, t] = STATUS[s] || STATUS.unknown; return badge(k, t, fixed); };
const kv = (k, v) => `<div class="kv"><span>${esc(k)}</span><span class="mono">${esc(fmt.dash(v))}</span></div>`;
const kvRaw = (k, html) => `<div class="kv"><span>${esc(k)}</span><span>${html}</span></div>`;
const bar = (pct, kind = '', thick = false) => `<div class="bar ${kind}${thick ? ' thick' : ''}" role="progressbar" aria-valuenow="${Math.round(pct)}" aria-valuemin="0" aria-valuemax="100"><i data-w="${Math.max(0, Math.min(100, pct))}"></i></div>`;
const pctKind = (p, warnAt = 75, badAt = 90) => p >= badAt ? 'bad' : p >= warnAt ? 'warn' : '';
const scoreColor = s => s >= 85 ? 'var(--accent)' : s >= 65 ? 'var(--warn-fg)' : 'var(--bad-fg)';
function ring(pct, size = 140, stroke = 12, color = 'var(--accent)', label = null) {
  const r = (size - stroke) / 2, c = 2 * Math.PI * r;
  return `<svg class="ring" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}" role="img" aria-label="${esc(label ?? Math.round(pct) + ' percent')}">
    <circle class="bg" cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke-width="${stroke}"/>
    <circle class="fg" cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="${color}" stroke-width="${stroke}" stroke-linecap="round"
      stroke-dasharray="${c}" stroke-dashoffset="${c}" data-pct="${pct}" data-c="${c}" transform="rotate(-90 ${size / 2} ${size / 2})"/>
    <text x="50%" y="50%" text-anchor="middle" dominant-baseline="central" font-size="${size * .26}">${esc(label ?? Math.round(pct))}</text></svg>`;
}
const skeleton = (h = 120) => `<div class="card"><div class="sk" style="height:${h}px"></div></div>`;
const errorCard = (id, msg) => `<div class="card"><h2>Could not read this section</h2><p class="hint">${esc(msg || 'Unknown error')}</p><div style="margin-top:12px"><button class="btn" data-rescan="${id}" type="button">Try again</button></div></div>`;
const pageHead = (title, sub, actions = '') => `<header class="page-head"><div><h1>${esc(title)}</h1><p>${sub}</p></div><div class="actions">${actions}</div></header>`;

/* ---------- state ---------- */
const DS = {
  data: {}, errors: {}, live: null, updates: null, lastScan: null,
  info: { windows: false, admin: false, settings: { theme: 'dark' } },
  scan: { running: false, progress: 0, steps: {}, log: [] },
  speed: { running: false, history: [], last: null }, publicIp: null,
  route: 'overview', dev: false,
};

// Positional arguments of each backend command (Tauri wants a named-argument object).
const API_ARGS = { scan: ['step'], set_settings: ['patch'], save_file: ['name', 'content', 'kind'] };
async function api(name, ...args) {
  const T = window.__TAURI__;
  if (T && T.core) {
    const keys = API_ARGS[name] || [];
    const payload = {};
    keys.forEach((k, i) => { if (args[i] !== undefined) payload[k] = args[i]; });
    return T.core.invoke(name, payload);
  }
  throw new Error('Backend not available (open the DeepScan app)');
}

/* ---------- toast + theme ---------- */
let toastTimer;
function toast(msg) {
  const t = $('#toast'); t.textContent = msg; t.classList.add('on');
  clearTimeout(toastTimer); toastTimer = setTimeout(() => t.classList.remove('on'), 2600);
}
function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  $('#themeBtn').innerHTML = icon(theme === 'dark' ? 'sun' : 'moon', 20);
  $('#themeBtn').title = theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme';
  if (DS.route === 'reports' && VIEWS.reports.after) VIEWS.reports.after($('#view'));
}

/* ---------- derived values ---------- */
function systemVolume() {
  const v = (DS.data.storage && DS.data.storage.volumes) || [];
  return v.find(x => x.letter === 'C') || v[0] || null;
}
function storageScore() {
  const d = (DS.data.storage && DS.data.storage.disks) || [];
  return Math.max(0, 100 - d.reduce((n, x) => n + (x.health === 'bad' ? 40 : x.health === 'warn' ? 20 : 0), 0));
}
function healthScore() {
  const sec = DS.data.security && DS.data.security.score != null ? DS.data.security.score : 100;
  let s = Math.round(.7 * sec + .3 * storageScore());
  if (DS.updates && DS.updates.ok && DS.updates.count > 0) s -= Math.min(8, DS.updates.count);
  return Math.max(0, s);
}
const healthWord = s => s >= 85 ? 'Good' : s >= 65 ? 'Needs attention' : 'At risk';
function findings() {
  const f = [], sec = DS.data.security, st = DS.data.storage, sw = DS.data.software;
  for (const kind of ['bad', 'warn']) {
    ((sec && sec.checks) || []).filter(c => c.status === kind).forEach(c => f.push({ kind, title: c.name + ': ' + c.detail, sub: c.advice, go: 'security' }));
    ((st && st.disks) || []).filter(d => d.health === kind).forEach(d => f.push({ kind, title: `${d.name} reports a problem`, sub: `Health: ${d.status}${d.read_errors ? ', ' + d.read_errors + ' read errors' : ''}. Back up important data.`, go: 'storage' }));
  }
  if (DS.updates && DS.updates.ok && DS.updates.count > 0) f.push({ kind: 'warn', title: `${DS.updates.count} Windows updates pending`, sub: DS.updates.security ? `${DS.updates.security} are security updates.` : '', go: 'security' });
  const on = sw && sw.startup ? sw.startup.filter(s => s.enabled).length : 0;
  if (on > 15) f.push({ kind: 'info', title: `${on} programs start with Windows`, sub: 'Turn off the ones you do not need in Task Manager > Startup apps.', go: 'software' });
  if (!f.length && (sec || st)) f.push({ kind: 'ok', title: 'No problems found', sub: 'Everything that was checked looks healthy.', go: null });
  return f;
}

/* ---------- router ---------- */
function paintBars(root) {
  requestAnimationFrame(() => requestAnimationFrame(() => {
    $$('.bar>i[data-w]', root).forEach(i => { i.style.width = i.dataset.w + '%'; });
    $$('.ring .fg', root).forEach(c => { const full = +c.dataset.c; c.style.strokeDashoffset = full * (1 - Math.min(100, +c.dataset.pct) / 100); });
    $$('[data-h]', root).forEach(e => { e.style.height = e.dataset.h + 'px'; });
  }));
}
function setNav(name) {
  $$('#nav a').forEach(a => a.getAttribute('href') === '#/' + name ? a.setAttribute('aria-current', 'page') : a.removeAttribute('aria-current'));
}
function render(name, animate = true) {
  const v = VIEWS[name]; if (!v) return;
  const el = $('#view'), y = window.scrollY;
  el.classList.remove('enter');
  el.innerHTML = v.render();
  if (animate) { void el.offsetWidth; el.classList.add('enter'); [...el.children].forEach((c, i) => c.style.setProperty('--i', i)); }
  paintBars(el);
  if (v.after) v.after(el);
  if (typeof updateChip === 'function') updateChip();
  setNav(name);
  document.title = (v.title || 'DeepScan') + ' - DeepScan';
  window.scrollTo(0, animate ? 0 : y);
}
function route() {
  let name = (location.hash.match(/^#\/(\w+)/) || [])[1] || 'overview';
  if (!VIEWS[name]) name = 'overview';
  DS.route = name; render(name);
  updateProgressLine();
}
function refresh() { render(DS.route, false); }

/* ---------- live metrics ---------- */
function applyLive() {
  const L = DS.live; if (!L) return;
  $$('[data-live]').forEach(el => {
    const k = el.dataset.live;
    if (k === 'cpu') el.textContent = L.cpu;
    else if (k === 'mem') el.textContent = L.mem;
    else if (k === 'mem_sub') el.textContent = `${L.mem_used_gb} of ${L.mem_total_gb} GB in use`;
    else if (k === 'uptime') el.textContent = fmt.uptime(L.uptime_s);
    else if (k === 'down') el.textContent = L.down_mbps;
    else if (k === 'up') el.textContent = L.up_mbps;
  });
  const cb = $('[data-livebar=cpu]'), mb = $('[data-livebar=mem]');
  if (cb) { cb.style.width = L.cpu + '%'; cb.parentElement.className = 'bar ' + pctKind(L.cpu, 70, 90); }
  if (mb) { mb.style.width = L.mem + '%'; mb.parentElement.className = 'bar ' + pctKind(L.mem, 80, 92); }
}
async function pollLive() {
  try { DS.live = await api('live'); applyLive(); } catch (e) { /* UI keeps last value */ }
}

/* ---------- scanning ---------- */
function updateProgressLine() {
  const p = $('#progress'), show = DS.scan.running && DS.route !== 'scan';
  p.classList.toggle('on', show); p.firstElementChild.style.width = DS.scan.progress + '%';
}
function scanLog(msg) { DS.scan.log.push(`${new Date().toLocaleTimeString([], { hour12: false })} ${msg}`); if (DS.scan.log.length > 40) DS.scan.log.shift(); }
function scanUI() { // update in place so the progress ring animates smoothly
  if (DS.route === 'scan') { if ($('#scanRoot')) VIEWS.scan.update(); else render('scan'); }
  updateProgressLine();
}

async function runScan(only) {
  const s = DS.scan; if (s.running) return;
  const steps = only ? STEPS.filter(x => x[0] === only) : STEPS;
  s.running = true; s.progress = 0; s.steps = {}; s.log = [];
  if (!only) { DS.data = {}; DS.errors = {}; DS.updates = null; }
  scanLog('Scan started');
  if (location.hash !== '#/scan') location.hash = '#/scan'; else scanUI();
  let done = 0;
  for (const [id, label] of steps) {
    s.steps[id] = 'running'; scanLog(`Checking ${label.toLowerCase()}...`); scanUI();
    const t = Date.now(); let d;
    try { d = await api('scan', id); } catch (e) { d = { error: String(e) }; }
    await sleep(Math.max(0, 450 - (Date.now() - t)));
    if (!d || d.error) { DS.errors[id] = (d && d.error) || 'No data'; delete DS.data[id]; s.steps[id] = 'failed'; scanLog(`${label}: failed`); }
    else { DS.data[id] = d; delete DS.errors[id]; s.steps[id] = 'done'; scanLog(`${label}: done in ${d._seconds}s`); }
    s.progress = Math.round(++done / steps.length * 100); scanUI();
  }
  s.running = false; DS.lastScan = new Date(); scanLog('Scan finished'); scanUI();
  await sleep(700);
  if (location.hash === '#/scan') location.hash = '#/overview'; else refresh();
  toast('Scan complete');
  updateProgressLine();
  checkUpdates();
}
async function checkUpdates() {
  try {
    const u = await api('updates'); DS.updates = u;
    const c = DS.data.security && DS.data.security.checks && DS.data.security.checks.find(x => x.id === 'updates');
    if (c) {
      if (u.ok) { c.status = u.count === 0 ? 'ok' : 'warn'; c.detail = u.count === 0 ? 'Up to date' : `${u.count} pending${u.security ? ' (' + u.security + ' security)' : ''}`; c.advice = u.count ? 'Open Settings > Windows Update and install them.' : ''; }
      else { c.status = 'unknown'; c.detail = 'Could not check (offline or blocked)'; }
      const sec = DS.data.security; sec.score = Math.max(0, 100 - 15 * sec.checks.filter(x => x.status === 'bad').length - 6 * sec.checks.filter(x => x.status === 'warn').length);
    }
    if (['overview', 'security'].includes(DS.route)) refresh();
  } catch (e) { /* ignore */ }
}

/* ---------- boot ---------- */
function buildChrome() {
  $('#nav').innerHTML = NAV.map(([id, label]) => `<a href="#/${id}">${icon(id)}<span class="label">${label}</span></a>`).join('');
  $('#themeBtn').addEventListener('click', async () => {
    const t = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
    applyTheme(t); try { await api('set_settings', { theme: t }); } catch (e) { /* ignore */ }
  });
  document.addEventListener('click', e => {
    const b = e.target.closest('[data-rescan]'); if (b) runScan(b.dataset.rescan);
    const g = e.target.closest('[data-go]'); if (g) location.hash = '#/' + g.dataset.go;
    const a = e.target.closest('[data-action]'); if (a && ACTIONS[a.dataset.action]) ACTIONS[a.dataset.action](a, e);
  });
  document.addEventListener('input', e => { const a = e.target.closest('[data-input]'); if (a && ACTIONS[a.dataset.input]) ACTIONS[a.dataset.input](a, e); });
  document.addEventListener('change', e => { const a = e.target.closest('[data-change]'); if (a && ACTIONS[a.dataset.change]) ACTIONS[a.dataset.change](a, e); });
  document.addEventListener('keydown', e => { // keyboard access for non-button controls such as sortable headers
    if ((e.key === 'Enter' || e.key === ' ') && e.target.matches('th[data-action]')) { e.preventDefault(); e.target.click(); }
  });
  window.addEventListener('hashchange', route);
}
function renderNotice() {
  const n = $('#notice'), s = DS.info.settings || {};
  if (DS.info.windows && !DS.info.admin && !s.hide_admin_notice) {
    n.innerHTML = `<div class="notice"><span>DeepScan is running without administrator rights, so BitLocker, SMBv1 and some other checks will show "Unknown". Close it and start it with "Run as administrator" for the full picture.</span><button class="btn" id="hideNotice" type="button">Got it</button></div>`;
    $('#hideNotice').onclick = async () => { n.innerHTML = ''; try { await api('set_settings', { hide_admin_notice: true }); } catch (e) { /* ignore */ } };
  } else if (!DS.info.windows) {
    n.innerHTML = `<div class="notice"><span>Demo mode: this is not Windows, so the app shows sample data.</span></div>`;
  } else n.innerHTML = '';
}
async function start() {
  buildChrome();
  try { DS.info = await api('app_info'); } catch (e) { /* defaults */ }
  applyTheme((DS.info.settings && DS.info.settings.theme) || 'dark');
  renderNotice();
  const dev = DS.info.windows ? '' : 'Demo data';
  $('#deviceChip').innerHTML = `<b id="chipName">This device</b><span id="chipSub">${dev || 'Ready to scan'}</span>`;
  try { DS.speed.history = await api('speed_history'); } catch (e) { /* none yet */ }
  pollLive(); setInterval(pollLive, 2000);
  route();
  if (!Object.keys(DS.data).length) { await sleep(250); runScan(); }
}
DS.boot = function () { start(); };
const ACTIONS = {};
const VIEWS = {};
