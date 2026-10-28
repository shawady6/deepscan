/* In-app updates: checks GitHub releases, shows a banner, downloads, verifies and restarts into the new version. */
DS.upd = { info: null, busy: '', checking: false };
const UP = DS.upd;

function updVersion() { return (DS.info && DS.info.version) || ''; }
function updSettings() { return (DS.info && DS.info.settings) || {}; }

function updPaint() {
  const row = $('#verRow');
  if (row) row.innerHTML = `<span>Version ${esc(updVersion())}</span><button class="link" data-action="updCheck" type="button" ${UP.checking ? 'disabled' : ''}>${UP.checking ? 'Checking...' : 'Check for updates'}</button>`;
  const bar = $('#updateBar'), i = UP.info;
  if (UP.busy) { bar.innerHTML = `<div class="notice" role="status"><span><span class="spinner"></span>&nbsp; ${esc(UP.busy)}</span></div>`; return; }
  if (i && i.ok && i.available && updSettings().skip_version !== i.latest) {
    bar.innerHTML = `<div class="notice" role="status"><span>DeepScan ${esc(i.latest)} is available. You have ${esc(i.current)}.</span>
      <span style="display:flex;gap:8px;flex-wrap:wrap"><button class="btn" data-action="updInstall" type="button">Update now</button><button class="btn" data-action="updNotes" type="button">What's new</button><button class="btn" data-action="updSkip" type="button">Skip this version</button></span></div>`;
  } else bar.innerHTML = '';
  if (DS.route === 'reports') render('reports', false);
}

async function updCheck(manual) {
  if (UP.checking) return;
  UP.checking = true; updPaint();
  let r;
  try { r = await api('update_check'); } catch (e) { r = { ok: false, error: String(e) }; }
  UP.checking = false; UP.info = r; updPaint();
  if (manual) {
    if (!r.ok) toast(r.none ? 'No published version yet, you have the latest.' : r.error || 'Could not check for updates');
    else if (!r.available) toast(`You have the latest version (${r.current})`);
    else if (updSettings().skip_version === r.latest) { await api('set_settings', { skip_version: '' }); DS.info.settings.skip_version = ''; updPaint(); }
  }
}
function updInit() {
  updPaint();
  if (DS.info && DS.info.windows && updSettings().auto_update_check !== false) setTimeout(() => updCheck(false), 4000);
}

ACTIONS.updCheck = () => updCheck(true);
ACTIONS.updNotes = () => {
  const i = UP.info; if (!i) return;
  infoBox(`What's new in ${i.latest}`, `<pre>${esc(i.notes || 'No release notes.')}</pre>`);
};
ACTIONS.updSkip = async () => {
  const i = UP.info; if (!i) return;
  try { DS.info.settings = await api('set_settings', { skip_version: i.latest }); } catch (e) { /* ignore */ }
  updPaint(); toast(`Skipped ${i.latest}. You can still update from Check for updates.`);
};
ACTIONS.updInstall = async () => {
  const i = UP.info; if (!i || UP.busy) return;
  if (!i.asset_url || !i.sha_url) {
    return infoBox('Cannot install automatically', `<p>This release has no verified installer file.</p><p class="hint">Download it from the release page instead:</p><pre>${esc(i.page || '')}</pre>`);
  }
  const mb = i.size ? ` (about ${Math.round(i.size / 1048576)} MB)` : '';
  if (!(await confirmBox({ title: `Update to ${i.latest}?`, ok: 'Download and restart',
    body: `<p>DeepScan will download the new version${mb}, check it, then close and start again. Windows may ask for administrator permission.</p><p class="hint">Save any report you are working on first.</p>` }))) return;
  UP.busy = 'Downloading and checking the update...'; updPaint();
  let r;
  try { r = await api('update_install', i.asset_url, i.sha_url); } catch (e) { r = { ok: false, error: String(e) }; }
  UP.busy = ''; updPaint();
  if (r && r.ok === false) infoBox('Update failed', `<p>${esc(r.error || 'Unknown error')}</p><p class="hint">Your current version was not changed.</p>`);
};
ACTIONS.updAuto = async el => {
  try { DS.info.settings = await api('set_settings', { auto_update_check: el.checked }); } catch (e) { /* ignore */ }
};

/* "About" card at the bottom of Reports (the sidebar link is hidden on narrow windows). */
(function () {
  const base = VIEWS.reports.render;
  VIEWS.reports.render = function () {
    const s = updSettings(), i = UP.info;
    const status = !i ? '' : !i.ok ? esc(i.error || '') : i.available ? `Version ${esc(i.latest)} is available.` : 'You have the latest version.';
    return base.call(this) + `<div class="card"><div class="card-head"><div class="stack" style="gap:2px"><h2 style="margin:0">DeepScan ${esc(updVersion())}</h2><span class="card-note">${status}</span></div>
      <button class="btn" data-action="updCheck" type="button" ${UP.checking ? 'disabled' : ''}>${UP.checking ? 'Checking...' : 'Check for updates'}</button></div>
      <label class="opt" style="margin-top:8px"><input type="checkbox" data-change="updAuto" ${s.auto_update_check !== false ? 'checked' : ''}>Check for updates when DeepScan starts</label></div>`;
  };
})();
