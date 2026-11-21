/* Navigation helpers: Quick fix by problem, favourites and recents, and the Ctrl+K search.
   Nothing here runs a tool by itself: choosing an item opens the same confirmation as pressing its Run button. */
const lsGet = (k, d) => { try { const v = JSON.parse(localStorage.getItem(k)); return v == null ? d : v; } catch (e) { return d; } };
const lsSet = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (e) { /* storage not available */ } };
let FAVS = new Set(lsGet('ds.fav', []));
let RECENT = lsGet('ds.recent', []);
const QK = { open: new Set() };
const keyOf = (kind, id) => kind + ':' + id;

function recentAdd(kind, id) {
  const k = keyOf(kind, id);
  RECENT = [k, ...RECENT.filter(x => x !== k)].slice(0, 6); lsSet('ds.recent', RECENT);
}
ACTIONS.favToggle = el => {
  const k = keyOf(el.dataset.kind, el.dataset.id);
  FAVS.has(k) ? FAVS.delete(k) : FAVS.add(k); lsSet('ds.fav', [...FAVS]);
  render(DS.route, false);
};

/* ---------- items (fixes and toolkit tools in one shape) ---------- */
function findItem(k) {
  const [kind, id] = k.split(':');
  if (kind === 'fix') {
    const f = ((RP.state && RP.state.fixes) || []).find(x => x.id === id);
    return f ? { kind, id, name: f.name, desc: f.desc, risk: f.risk, runnable: f.runnable, group: 'Common fixes' } : null;
  }
  const tk = RP.tk || {}, t = (tk.tools || []).find(x => x.id === id);
  if (!t) return null;
  const g = (tk.groups || []).find(x => x.id === t.group);
  return { kind: 'tool', id, name: t.name, desc: t.desc, risk: t.risk, group: g ? g.name : '' };
}
function allItems() {
  const fixes = ((RP.state && RP.state.fixes) || []).map(f => findItem('fix:' + f.id));
  const tools = ((RP.tk && RP.tk.tools) || []).map(t => findItem('tool:' + t.id));
  return [...fixes, ...tools].filter(Boolean);
}
function itemCard(o) {
  const star = FAVS.has(keyOf(o.kind, o.id)), act = o.kind === 'fix' ? 'rpFix' : 'rpTool', scr = o.kind === 'fix' ? 'rpScript' : 'rpToolScript';
  return `<div class="rp-fix ${o.risk === 'high' ? 'r-high' : ''}"><div class="top"><b>${esc(o.name)}</b><span class="top-r">${riskBadge(o.risk)}<button class="star" data-action="favToggle" data-kind="${o.kind}" data-id="${o.id}" type="button" aria-pressed="${star}" aria-label="${star ? 'Remove from favorites' : 'Add to favorites'}" title="${star ? 'Remove from favorites' : 'Add to favorites'}">${star ? '★' : '☆'}</button></span></div>
    <span class="hint">${esc(o.desc)}</span>
    <div class="btns">${o.runnable !== false ? `<button class="btn small primary" data-action="${act}" data-id="${o.id}" type="button" ${RP.busy ? 'disabled' : ''}>Run</button>` : ''}<button class="btn small" data-action="${scr}" data-id="${o.id}" type="button">View script</button></div></div>`;
}

/* ---------- Quick fix: start from the problem ---------- */
const PROBLEMS = [
  { id: 'net', title: 'Internet is slow or not working', sub: 'DNS, proxy, adapters, TCP', items: ['tool:net_test', 'tool:net_boost', 'tool:adapters_restart', 'tool:hosts_proxy', 'tool:proxy_reset', 'fix:net_reset', 'tool:time_sync', 'tool:tcpip_reset'] },
  { id: 'slow', title: 'The PC is slow', sub: 'Startup, cleanup, power plan, disk', items: ['tool:startup_list', 'tool:clean_temp', 'tool:deep_clean', 'fix:disk100', 'tool:power_high', 'tool:faststart_off', 'tool:defrag', 'tool:sysmain_off'] },
  { id: 'space', title: 'The disk is full', sub: 'Find big files and clean caches', items: ['tool:big_files', 'tool:clean_temp', 'tool:deep_clean', 'tool:recycle', 'tool:clean_caches'] },
  { id: 'wu', title: 'Windows Update problems', sub: 'Stuck, failing or unwanted updates', items: ['fix:wu_reset', 'tool:wu_settings', 'tool:wu_start', 'tool:wu_stop', 'tool:dism_check', 'fix:sfc_dism', 'tool:time_sync'] },
  { id: 'shell', title: 'Apps, Start menu or taskbar broken', sub: 'Shell, Search, Store and app packages', items: ['fix:shell_reset', 'fix:search_reset', 'fix:store_reset', 'fix:appx_fix', 'tool:appx_all', 'tool:explorer_restart', 'tool:wsreset', 'fix:icons'] },
  { id: 'files', title: 'Windows files may be corrupted', sub: 'Blue screens, odd errors, failed updates', items: ['tool:sfc_verify', 'tool:dism_check', 'tool:dism_scan', 'fix:sfc_dism', 'fix:wmi_fix', 'tool:chkdsk_scan', 'tool:chkdsk_repair', 'tool:memdiag'] },
  { id: 'dev', title: 'Printer, sound or Bluetooth not working', sub: 'Device services and drivers', items: ['fix:printer', 'fix:audio', 'fix:bluetooth'] },
  { id: 'power', title: 'Battery and power', sub: 'Battery health and power plans', items: ['tool:battery', 'tool:energy', 'tool:power_high', 'tool:power_balanced', 'tool:faststart_off'] },
  { id: 'sec', title: 'Security check', sub: 'Defender, firewall, Remote Desktop, activation', items: ['tool:sec_snapshot', 'tool:def_update_scan', 'tool:rdp_off', 'tool:activation'] },
];
document.addEventListener('toggle', e => {
  const d = e.target; if (d && d.matches && d.matches('details.qk-group')) d.open ? QK.open.add(d.dataset.qk) : QK.open.delete(d.dataset.qk);
}, true);

VIEWS.quick = {
  title: 'Quick fix',
  after() { rpInit(false); },
  render() {
    const head = pageHead('Quick fix', 'Pick the problem, then run the tools that usually fix it. Nothing runs until you press Run. Press Ctrl+K to search everything.');
    if (!RP.state || !RP.tk) return head + skeleton(320);
    const busy = RP.busy ? `<div class="notice" role="status"><span><span class="spinner"></span>&nbsp; ${esc(RP.busy)}</span></div>` : '';
    const list = keys => keys.map(findItem).filter(Boolean);
    const sec = (title, keys) => { const it = list(keys); return it.length ? `<div class="card qk-sec"><h2>${title}</h2><div class="rp-grid">${it.map(itemCard).join('')}</div></div>` : ''; };
    const problems = PROBLEMS.map(p => {
      const it = list(p.items); if (!it.length) return '';
      return `<details class="qk-group" data-qk="${p.id}" ${QK.open.has(p.id) ? 'open' : ''}><summary>${esc(p.title)}<span class="sub">${esc(p.sub)}</span></summary><div class="rp-grid">${it.map(itemCard).join('')}</div></details>`;
    }).join('');
    return head + busy + sec('Favorites', [...FAVS]) + sec('Recently used', RECENT) + `<div class="card"><h2>What is wrong?</h2>${problems}</div>`;
  },
};

/* ---------- Ctrl+K search ---------- */
const TABS = [['overview', 'Overview'], ['hardware', 'Hardware'], ['storage', 'Storage'], ['software', 'Software'], ['devices', 'Devices'], ['network', 'Network'],
  ['security', 'Security'], ['quick', 'Quick fix'], ['repair', 'Repair'], ['toolkit', 'Toolkit'], ['reports', 'Reports']];
// Extra words people type for a problem, so "wifi" finds the network tools and "lag" finds the cleanup ones.
const KW = {
  net_boost: 'wifi wi-fi internet slow lag dns speed ping connection', net_test: 'wifi internet ping dns gateway offline', adapters_restart: 'wifi ethernet adapter network card',
  proxy_reset: 'proxy vpn internet', hosts_proxy: 'hosts proxy redirect', hosts_reset: 'hosts redirect virus', tcpip_reset: 'tcp ip winsock network reset', time_sync: 'clock date time',
  deep_clean: 'clean cleanup cache temp junk space slow', clean_temp: 'clean temp junk space', clean_caches: 'cache icon thumbnail', recycle: 'recycle bin trash delete', big_files: 'large big space disk full',
  sysmain_off: 'superfetch slow disk 100', power_high: 'performance slow power', battery: 'battery health wear laptop', energy: 'battery power efficiency drain', activation: 'license key windows activate',
  sfc_verify: 'corrupt system files sfc', dism_check: 'corrupt component store dism', dism_scan: 'corrupt component store dism', chkdsk_scan: 'disk errors bad sectors', chkdsk_repair: 'disk errors bad sectors',
  memdiag: 'ram memory test crash blue screen', appx_all: 'store apps broken', wsreset: 'store app download', explorer_restart: 'taskbar desktop frozen',
  wu_stop: 'update disable stop', wu_start: 'update enable start', wu_settings: 'update policy', def_update_scan: 'virus malware antivirus scan', sec_snapshot: 'security firewall defender uac',
  rdp_on: 'remote desktop', rdp_off: 'remote desktop', winget_all: 'update apps upgrade', driver_backup: 'drivers backup export', office_repair: 'word excel outlook office repair',
  safemode_on: 'safe mode boot', safemode_off: 'safe mode boot', adv_startup: 'recovery boot startup repair', startup_list: 'startup boot slow programs', diag: 'diagnosis health report',
  printer: 'printer print spooler', audio: 'sound speaker microphone', bluetooth: 'bluetooth wireless pair', net_reset: 'internet dns winsock', disk100: 'disk 100 slow', wu_reset: 'windows update stuck failing',
  shell_reset: 'start menu taskbar broken', search_reset: 'search not working', store_reset: 'store apps', appx_fix: 'apps broken', icons: 'icons blank thumbnails', sfc_dism: 'corrupt repair system files',
};
const PAL = { el: null, rows: [], sel: 0 };
function palRows(q) {
  const tabs = TABS.map(([id, n]) => ({ type: 'tab', id, name: n, sub: 'Open this screen', hay: (n + ' ' + id).toLowerCase() }));
  const probs = PROBLEMS.map(p => ({ type: 'problem', id: p.id, name: p.title, sub: p.sub, hay: (p.title + ' ' + p.sub).toLowerCase() }));
  const items = allItems().map(o => ({ type: o.kind, id: o.id, name: o.name, sub: o.group + ' - ' + o.desc, risk: o.risk, runnable: o.runnable, hay: (o.name + ' ' + o.group + ' ' + o.desc + ' ' + (KW[o.id] || '')).toLowerCase() }));
  const pool = [...items, ...probs, ...tabs], terms = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) {
    const fav = [...FAVS, ...RECENT].map(findItem).filter(Boolean).filter((o, i, a) => a.findIndex(x => x.kind === o.kind && x.id === o.id) === i)
      .map(o => ({ type: o.kind, id: o.id, name: o.name, sub: 'Favorite or recent - ' + o.group, risk: o.risk, runnable: o.runnable }));
    return [...fav.slice(0, 6), ...tabs];
  }
  return pool.map(r => {
    let score = 0;
    for (const t of terms) {
      const nm = r.name.toLowerCase();
      if (!r.hay.includes(t)) return null;
      score += nm === t ? 8 : nm.startsWith(t) ? 4 : nm.includes(t) ? 3 : 1;
    }
    return { r, score };
  }).filter(Boolean).sort((a, b) => b.score - a.score).slice(0, 14).map(x => x.r);
}
const PAL_KIND = { tab: 'Screen', problem: 'Problem', fix: 'Fix', tool: 'Tool' };
function palPaint() {
  const q = PAL.el.querySelector('input').value;
  PAL.rows = palRows(q); PAL.sel = Math.min(PAL.sel, Math.max(0, PAL.rows.length - 1));
  PAL.el.querySelector('ul').innerHTML = PAL.rows.length ? PAL.rows.map((r, i) => `<li role="option" data-i="${i}" aria-selected="${i === PAL.sel}"><span class="k">${PAL_KIND[r.type]}</span><span class="n">${esc(r.name)}<small>${esc(r.sub || '')}</small></span>${r.risk ? riskBadge(r.risk) : ''}</li>`).join('')
    : `<li class="empty">Nothing matches "${esc(q)}"</li>`;
  const cur = PAL.el.querySelector('li[aria-selected=true]'); if (cur && cur.scrollIntoView) cur.scrollIntoView({ block: 'nearest' });
}
async function palOpen() {
  if (PAL.el) return;
  const back = document.createElement('div'); back.className = 'modal-back pal-back';
  back.innerHTML = `<div class="pal" role="dialog" aria-modal="true" aria-label="Search"><input type="text" placeholder="Search tools, fixes and screens (try: wifi, slow, printer, battery)" aria-label="Search" autocomplete="off" spellcheck="false"><ul role="listbox"></ul><div class="foot"><span>Up and Down to move</span><span>Enter to open</span><span>Esc to close</span></div></div>`;
  PAL.el = back; PAL.sel = 0; document.body.appendChild(back);
  const inp = back.querySelector('input'); inp.focus();
  palPaint();
  rpInit(false).then(() => { if (PAL.el === back) palPaint(); }); // loads the static tool lists once; runs nothing
  inp.addEventListener('input', () => { PAL.sel = 0; palPaint(); });
  inp.addEventListener('keydown', e => {
    if (e.key === 'ArrowDown') { e.preventDefault(); PAL.sel = Math.min(PAL.rows.length - 1, PAL.sel + 1); palPaint(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); PAL.sel = Math.max(0, PAL.sel - 1); palPaint(); }
    else if (e.key === 'Enter') { e.preventDefault(); const r = PAL.rows[PAL.sel]; if (r) palChoose(r); }
    else if (e.key === 'Escape') { e.preventDefault(); palClose(); }
  });
  back.addEventListener('click', e => {
    const li = e.target.closest('li[data-i]'); if (li) palChoose(PAL.rows[+li.dataset.i]);
    else if (e.target === back) palClose();
  });
}
function palClose() { if (PAL.el) { PAL.el.remove(); PAL.el = null; } }
async function palChoose(r) {
  palClose();
  if (r.type === 'tab') { location.hash = '#/' + r.id; return; }
  if (r.type === 'problem') { QK.open.add(r.id); if (location.hash === '#/quick') render('quick', false); else location.hash = '#/quick'; return; }
  // Run through the normal confirmation. Show the Quick fix screen first so the running notice is visible.
  if (!['repair', 'toolkit', 'quick'].includes(DS.route)) { location.hash = '#/quick'; await sleep(80); }
  const el = { dataset: { id: r.id } };
  // Low-risk tools normally start without asking. From the search box a stray Enter must not start anything, so always ask once.
  if (r.risk === 'low' && !(await confirmBox({ title: r.name, ok: 'Run', body: `<p>${esc(r.sub.split(' - ').slice(1).join(' - ') || r.name)}</p>` }))) return;
  if (r.type === 'fix') (r.runnable === false ? ACTIONS.rpScript : ACTIONS.rpFix)(el);
  else ACTIONS.rpTool(el);
}
document.addEventListener('keydown', e => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); PAL.el ? palClose() : palOpen(); }
});
{ const b = document.getElementById('searchBtn'); if (b) b.addEventListener('click', palOpen); }
