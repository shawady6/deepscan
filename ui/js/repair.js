/* Repair tab: Windows health, bloatware, slim-down tweaks, fix scripts, restore point and undo history. */
DS.repair = {
  started: false, state: null, checks: {}, restore: null, bloat: null, bloatErr: '',
  selApps: new Set(), selTweaks: new Set(), preset: 'balanced', autoRestore: true, busy: '', output: null,
};
const RP = DS.repair;
const RISK = { low: ['ok', 'Low risk'], medium: ['warn', 'Medium risk'], high: ['bad', 'High risk'] };
const riskBadge = r => badge(...(RISK[r] || RISK.low));
const PRESETS = [
  ['light', 1, 'Light', 'Privacy and ads only. No risk to features.'],
  ['balanced', 2, 'Balanced', 'Recommended. Light plus Widgets, Copilot and background apps.'],
  ['aggressive', 3, 'Aggressive', 'Also stops services. Some features stop working.'],
];
const presetLevel = () => (PRESETS.find(p => p[0] === RP.preset) || PRESETS[1])[1];

/* ---------- dialogs ---------- */
function dialog(html, buttons) {
  return new Promise(resolve => {
    const back = document.createElement('div');
    back.className = 'modal-back';
    back.innerHTML = `<div class="modal" role="dialog" aria-modal="true">${html}<div class="acts">${buttons.map((b, i) => `<button class="btn ${b.cls || ''}" data-i="${i}" type="button">${esc(b.label)}</button>`).join('')}</div></div>`;
    const prev = document.activeElement;
    const done = v => { back.remove(); document.removeEventListener('keydown', onKey); if (prev && prev.focus) prev.focus(); resolve(v); };
    const onKey = e => { if (e.key === 'Escape') done(false); };
    back.addEventListener('click', e => { const b = e.target.closest('[data-i]'); if (b) done(buttons[+b.dataset.i].value); else if (e.target === back) done(false); });
    document.addEventListener('keydown', onKey);
    document.body.appendChild(back);
    const last = back.querySelectorAll('.btn'); last[last.length - 1].focus();
  });
}
const confirmBox = ({ title, body, ok = 'Continue', danger = false }) =>
  dialog(`<h2>${esc(title)}</h2><div class="body">${body}</div>`, [{ label: 'Cancel', value: false }, { label: ok, value: true, cls: danger ? 'danger' : 'primary' }]);
const infoBox = (title, html) => dialog(`<h2>${esc(title)}</h2><div class="body">${html}</div>`, [{ label: 'Close', value: true, cls: 'primary' }]);
const list = items => `<ul>${items.map(i => `<li>${esc(i)}</li>`).join('')}</ul>`;

/* ---------- data ---------- */
async function rpBackend(name, ...args) {
  try { return await api(name, ...args); } catch (e) { return { ok: false, error: String(e), output: String(e) }; }
}
async function rpLoadState() {
  RP.state = await rpBackend('repair_state');
  if (RP.state && RP.state.tweaks) {
    const lvl = presetLevel();
    RP.selTweaks = new Set(RP.state.tweaks.filter(t => t.level <= lvl && !t.applied).map(t => t.id));
  }
}
async function rpRunCheck(id) {
  RP.checks[id] = { status: 'running' }; rpPaint();
  RP.checks[id] = await rpBackend('health_check', id);
  if (!RP.checks[id].status) RP.checks[id] = { id, status: 'unknown', detail: RP.checks[id].error || 'Could not check' };
  rpPaint();
}
async function rpLoadBloat() {
  RP.bloat = null; RP.bloatErr = ''; rpPaint();
  const r = await rpBackend('bloat_scan');
  if (r.error) RP.bloatErr = r.error; else RP.bloat = r.items || [];
  RP.selApps = new Set();
  rpPaint();
}
async function rpInit(force) {
  if (RP.started && !force) return;
  RP.started = true;
  await rpLoadState(); rpPaint();
  rpBackend('restore_info').then(r => { RP.restore = r; rpPaint(); });
  rpLoadBloat();
  const fast = (RP.state.checks || []).filter(c => !c.slow);
  RP.checks = {};
  await Promise.all(fast.map(c => rpRunCheck(c.id)));
}
function rpPaint() {
  if (DS.route !== 'repair') return;
  const a = document.activeElement, key = a && a.closest && a.closest('[data-id],[data-change],[data-action]');
  const sel = key && (key.dataset.change ? `[data-change="${key.dataset.change}"]` : `[data-action="${key.dataset.action}"]`) + (key.dataset.id ? `[data-id="${key.dataset.id}"]` : (key.value ? `[value="${key.value}"]` : ''));
  render('repair', false);
  if (sel) { const n = document.querySelector(sel); if (n && n.focus) n.focus({ preventScroll: true }); }
}

/* ---------- guards ---------- */
async function rpRestoreFirst() {
  if (!RP.autoRestore) return true;
  RP.busy = 'Creating a restore point...'; rpPaint();
  const r = await rpBackend('restore_create');
  RP.busy = ''; rpPaint();
  if (r.ok) { rpBackend('restore_info').then(x => { RP.restore = x; rpPaint(); }); return true; }
  return confirmBox({ title: 'Could not create a restore point', ok: 'Continue without it', danger: true,
    body: `<p>${esc(r.message || r.error || 'Unknown error')}</p><p>You can continue, but these changes will not be covered by a restore point.</p>` });
}

/* ---------- view ---------- */
VIEWS.repair = {
  title: 'Repair',
  after() { rpInit(false); },
  render() {
    const st = RP.state;
    const head = pageHead('Repair and optimize', 'Check your Windows installation, fix problems, remove junk apps and slim it down.',
      `<button class="btn" data-action="rpRestorePoint" type="button" ${RP.busy ? 'disabled' : ''}>Create restore point</button>`);
    if (!st) return head + skeleton(320);
    const busy = RP.busy ? `<div class="notice" role="status"><span><span class="spinner"></span>&nbsp; ${esc(RP.busy)}</span></div>` : '';
    return head + busy + rpHealth() + `<div class="row">${rpSafety()}${rpBloat()}</div>` + rpSlim() + rpFixes() + rpHistory();
  },
};

function rpHealth() {
  const rows = (RP.state.checks || []).map(c => {
    const r = RP.checks[c.id];
    let badgeHtml, detail, act = '';
    if (!r) {
      badgeHtml = c.slow ? badge('info', 'Not run', true) : badge('info', 'Checking', true);
      detail = c.slow ? 'Takes several minutes. Run it when you have time.' : 'Checking...';
      if (c.slow) act = `<button class="btn small" data-action="rpCheck" data-id="${c.id}" type="button">Run scan</button>`;
    } else if (r.status === 'running') {
      badgeHtml = `<span class="badge info fixed"><span class="spinner"></span></span>`; detail = 'Checking...';
    } else {
      badgeHtml = statusBadge(r.status); detail = r.detail || '';
      if (r.advice && r.status !== 'ok') detail += '. ' + r.advice;
      if (r.fix === 'restart') act = `<button class="btn small" data-action="rpRestart" type="button">Restart now</button>`;
      else if (r.fix) act = `<button class="btn small primary" data-action="rpFix" data-id="${esc(r.fix)}" type="button">Repair</button>`;
      else if (c.slow) act = `<button class="btn small" data-action="rpCheck" data-id="${c.id}" type="button">Run again</button>`;
    }
    return `<div class="item">${badgeHtml}<div class="grow"><span class="title">${esc(c.name)}</span><span class="sub">${esc(detail)}</span></div>${act}</div>`;
  }).join('');
  return `<div class="card"><div class="card-head"><h2>Windows health</h2><button class="btn small" data-action="rpRecheck" type="button">Check again</button></div>${rows}</div>`;
}

function rpSafety() {
  const last = RP.restore ? (RP.restore.last ? esc(RP.restore.last) : 'None yet') : 'Checking...';
  return `<div class="card grow3"><h2>Safety</h2>
    <label class="rp-opt"><input type="checkbox" data-change="rpAuto" ${RP.autoRestore ? 'checked' : ''}><span class="grow"><span>Create a restore point before changes</span></span></label>
    <div class="rp-opt"><span class="grow" style="cursor:default"><span>Last restore point</span><span class="sub">${last}</span></span><button class="btn small" data-action="rpOpenRestore" type="button">Open</button></div>
    <p class="hint" style="margin-top:12px">Every change is shown first and needs your confirmation. Settings changes can be undone from History. Core Windows, the Microsoft Store, Defender and drivers are never removed, and Windows Update is never turned off.</p></div>`;
}

function rpBloat() {
  let body;
  if (RP.bloatErr) body = `<p class="hint">${esc(RP.bloatErr)}</p><button class="btn small" data-action="rpRescanApps" type="button">Try again</button>`;
  else if (!RP.bloat) body = `<p class="hint"><span class="spinner"></span>&nbsp; Looking for removable apps...</p>`;
  else if (!RP.bloat.length) body = `<p class="hint">No removable preinstalled apps found. Nothing to clean.</p>`;
  else {
    const rows = RP.bloat.map(a => `<label class="rp-opt"><input type="checkbox" data-change="rpApp" data-id="${esc(a.id)}" ${RP.selApps.has(a.id) ? 'checked' : ''}>
      <span class="grow"><span>${esc(a.name)}</span><span class="sub">${esc(a.maker)}${a.note ? ' · ' + esc(a.note) : ''}</span></span>${a.safety === 'safe' ? badge('ok', 'Safe', true) : badge('warn', 'Review', true)}</label>`).join('');
    const n = RP.selApps.size;
    body = `<div class="rp-bar" style="margin:0 0 4px"><button class="btn small" data-action="rpSelSafe" type="button">Select safe</button><button class="btn small" data-action="rpSelNone" type="button">Clear</button></div>${rows}
      <div class="rp-bar"><span class="grow">${n} selected</span><button class="btn primary" data-action="rpRemoveApps" type="button" ${n && !RP.busy ? '' : 'disabled'}>Remove selected</button></div>`;
  }
  return `<div class="card grow2"><h2>Remove bloatware</h2><p class="hint" style="margin-bottom:8px">Only known preinstalled apps are listed. Removed apps can be installed again from the Microsoft Store.</p>${body}</div>`;
}

function rpSlim() {
  const T = RP.state.tweaks || [];
  const presets = `<div class="rp-seg" role="radiogroup" aria-label="Preset">${PRESETS.map(p => `<label><input type="radio" name="rpPreset" value="${p[0]}" data-change="rpPreset" ${RP.preset === p[0] ? 'checked' : ''}><b>${p[2]}</b><span>${p[3]}</span></label>`).join('')}</div>`;
  const rows = T.map(t => `<label class="rp-opt"><input type="checkbox" data-change="rpTweak" data-id="${t.id}" ${RP.selTweaks.has(t.id) ? 'checked' : ''} ${t.applied ? 'disabled' : ''}>
    <span class="grow"><span>${esc(t.name)}</span><span class="sub">${esc(t.note)}</span></span>${t.applied ? badge('info', 'Already on', true) : riskBadge(t.risk)}</label>`).join('');
  const n = [...RP.selTweaks].filter(id => T.some(t => t.id === id && !t.applied)).length;
  return `<div class="card"><h2>Slim down Windows</h2><p class="hint">Pick a preset, then adjust single options if you want. Changes can be undone from History.</p>${presets}${rows}
    <div class="rp-bar"><span class="grow">${n} change${n === 1 ? '' : 's'} selected</span><button class="btn primary" data-action="rpApplyTweaks" type="button" ${n && !RP.busy ? '' : 'disabled'}>Apply ${n} change${n === 1 ? '' : 's'}</button></div></div>`;
}

function rpFixes() {
  const cards = (RP.state.fixes || []).map(f => `<div class="rp-fix"><div class="top"><b>${esc(f.name)}</b>${riskBadge(f.risk)}</div><span class="hint">${esc(f.desc)}</span>
    <div class="btns">${f.runnable ? `<button class="btn small primary" data-action="rpFix" data-id="${f.id}" type="button" ${RP.busy ? 'disabled' : ''}>Run</button>` : ''}<button class="btn small" data-action="rpScript" data-id="${f.id}" type="button">View script</button></div></div>`).join('');
  return `<div class="card"><h2>Fix common Windows problems</h2><p class="hint" style="margin-bottom:16px">Every script can be read before it runs. Medium and high risk fixes always ask for confirmation.</p><div class="rp-grid">${cards}</div></div>`;
}

function rpHistory() {
  const H = (RP.state.history || []);
  const rows = H.length ? H.map(h => `<div class="item"><div class="grow"><span class="title">${esc(h.title)}</span><span class="sub">${esc(h.ts)}${h.detail ? ' · ' + esc(h.detail) : ''}${h.note ? ' · ' + esc(h.note) : ''}</span></div>
    ${h.undone ? badge('info', 'Undone', true) : h.undoable ? `<button class="btn small" data-action="rpUndo" data-id="${esc(h.id)}" type="button" ${RP.busy ? 'disabled' : ''}>Undo</button>` : ''}</div>`).join('')
    : `<p class="hint">Nothing yet. Changes you make here will be listed so you can undo them.</p>`;
  return `<div class="card"><h2>History and undo</h2>${rows}</div>`;
}

/* ---------- actions ---------- */
ACTIONS.rpRecheck = async () => { RP.checks = {}; rpPaint(); await Promise.all((RP.state.checks || []).filter(c => !c.slow).map(c => rpRunCheck(c.id))); };
ACTIONS.rpCheck = el => rpRunCheck(el.dataset.id);
ACTIONS.rpAuto = el => { RP.autoRestore = el.checked; };
ACTIONS.rpApp = el => { el.checked ? RP.selApps.add(el.dataset.id) : RP.selApps.delete(el.dataset.id); rpPaint(); };
ACTIONS.rpSelSafe = () => { RP.selApps = new Set((RP.bloat || []).filter(a => a.safety === 'safe').map(a => a.id)); rpPaint(); };
ACTIONS.rpSelNone = () => { RP.selApps = new Set(); rpPaint(); };
ACTIONS.rpRescanApps = () => rpLoadBloat();
ACTIONS.rpTweak = el => { el.checked ? RP.selTweaks.add(el.dataset.id) : RP.selTweaks.delete(el.dataset.id); rpPaint(); };
ACTIONS.rpPreset = el => {
  RP.preset = el.value; const lvl = presetLevel();
  RP.selTweaks = new Set((RP.state.tweaks || []).filter(t => t.level <= lvl && !t.applied).map(t => t.id)); rpPaint();
};
ACTIONS.rpOpenRestore = () => rpBackend('restore_open');
ACTIONS.rpRestorePoint = async () => {
  RP.busy = 'Creating a restore point...'; rpPaint();
  const r = await rpBackend('restore_create');
  RP.busy = ''; await rpLoadState(); rpPaint();
  if (r.ok) { toast('Restore point created'); rpBackend('restore_info').then(x => { RP.restore = x; rpPaint(); }); }
  else infoBox('Could not create a restore point', `<p>${esc(r.message || r.error || 'Unknown error')}</p><p class="hint">System Protection must be available for the system drive, and DeepScan must run as administrator.</p>`);
};
ACTIONS.rpRestart = async () => {
  if (await confirmBox({ title: 'Restart the PC?', ok: 'Restart in 15 seconds', body: '<p>Save your work first. Windows will restart in 15 seconds.</p>' })) rpBackend('restart_now');
};
ACTIONS.rpScript = async el => {
  const f = (RP.state.fixes || []).find(x => x.id === el.dataset.id); const r = await rpBackend('fix_script', el.dataset.id);
  infoBox(f ? f.name : 'Script', `<p class="hint" style="margin-bottom:12px">PowerShell, as DeepScan would run it:</p><pre>${esc(r.script || 'Not available')}</pre>`);
};
ACTIONS.rpFix = async el => {
  const id = el.dataset.id, f = (RP.state.fixes || []).find(x => x.id === id); if (!f || RP.busy) return;
  const heavy = f.risk !== 'low';
  const ok = await confirmBox({ title: f.name, ok: 'Run fix', danger: heavy,
    body: `<p>${esc(f.desc)}</p>${heavy ? '<p><b>This fix changes system components.</b> A restore point is recommended.</p>' : ''}${id === 'sfc_dism' ? '<p class="hint">This can take a long time. Keep the app open.</p>' : ''}` });
  if (!ok) return;
  if (heavy && !(await rpRestoreFirst())) return;
  RP.busy = `Running: ${f.name}...`; rpPaint();
  const r = await rpBackend('fix_run', id);
  RP.busy = ''; await rpLoadState(); rpPaint();
  infoBox(r.ok ? `${f.name}: done` : `${f.name}: finished with errors`, `<pre>${esc(r.output || '(no output)')}</pre>`);
  ACTIONS.rpRecheck();
};
ACTIONS.rpRemoveApps = async () => {
  const items = (RP.bloat || []).filter(a => RP.selApps.has(a.id)); if (!items.length || RP.busy) return;
  const review = items.filter(a => a.safety === 'review');
  const ok = await confirmBox({ title: `Remove ${items.length} app${items.length === 1 ? '' : 's'}?`, ok: 'Remove', danger: true,
    body: `${list(items.map(a => a.name))}${review.length ? `<p><b>Check these first:</b> ${esc(review.map(a => a.name).join(', '))}. You may still use them.</p>` : ''}<p class="hint">Apps can be installed again from the Microsoft Store. They are removed for all users and will not come back for new accounts.</p>` });
  if (!ok || !(await rpRestoreFirst())) return;
  RP.busy = 'Removing apps...'; rpPaint();
  const r = await rpBackend('bloat_remove', items.map(a => a.id));
  RP.busy = ''; await rpLoadState(); await rpLoadBloat();
  toast(r.ok ? `Removed ${r.removed} app${r.removed === 1 ? '' : 's'}` : `Some apps could not be removed (${(r.failed || []).length})`);
};
ACTIONS.rpApplyTweaks = async () => {
  const T = RP.state.tweaks || [], items = T.filter(t => RP.selTweaks.has(t.id) && !t.applied); if (!items.length || RP.busy) return;
  const high = items.filter(t => t.risk === 'high');
  const ok = await confirmBox({ title: `Apply ${items.length} change${items.length === 1 ? '' : 's'}?`, ok: 'Apply', danger: high.length > 0,
    body: `${list(items.map(t => t.name))}${high.length ? `<p><b>High risk:</b> ${esc(high.map(t => t.name).join(', '))}. Read the notes above before you continue.</p>` : ''}<p class="hint">You can undo these from History. Some changes need a sign out or restart to take effect.</p>` });
  if (!ok || !(await rpRestoreFirst())) return;
  RP.busy = 'Applying changes...'; rpPaint();
  const r = await rpBackend('tweaks_apply', items.map(t => t.id));
  RP.busy = ''; await rpLoadState(); rpPaint();
  const failed = (r.results || []).filter(x => !x.ok);
  if (failed.length) infoBox('Some changes failed', `${list(failed.map(x => `${(T.find(t => t.id === x.id) || {}).name || x.id}: ${x.error || 'error'}`))}<p class="hint">Administrator rights are needed for some of these.</p>`);
  else toast('Changes applied. Sign out or restart to finish.');
};
ACTIONS.rpUndo = async el => {
  const h = (RP.state.history || []).find(x => x.id === el.dataset.id); if (!h || RP.busy) return;
  if (!(await confirmBox({ title: 'Undo this change?', ok: 'Undo', body: `<p>${esc(h.title)}</p><p class="hint">${esc(h.detail || '')}</p>` }))) return;
  RP.busy = 'Undoing...'; rpPaint();
  const r = await rpBackend('history_undo', h.id);
  RP.busy = ''; await rpLoadState(); rpPaint();
  toast(r.ok ? 'Change undone' : 'Could not undo: ' + (r.error || 'unknown error'));
};
