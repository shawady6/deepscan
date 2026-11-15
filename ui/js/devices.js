/* Devices tab: battery health and USB flash drives (check, format). Nothing runs until a button is pressed. */
DS.dev = { battery: null, drives: null, busy: '', batBusy: false, listBusy: false };
const DV = DS.dev;
const dvCall = async (name, ...a) => { try { return await api(name, ...a); } catch (e) { return { ok: false, error: String(e), output: String(e) }; } };
const dvPaint = () => { if (DS.route === 'devices') render('devices', false); };
const dvFmt = n => n == null ? '-' : n + ' GB';

VIEWS.devices = {
  title: 'Devices',
  render() {
    const head = pageHead('Devices', 'Battery health and USB flash drives. Nothing is read or changed until you press a button.');
    const busy = DV.busy ? `<div class="notice" role="status"><span><span class="spinner"></span>&nbsp; ${esc(DV.busy)}</span></div>` : '';
    return head + busy + dvBattery() + dvDrives();
  },
};

function dvBattery() {
  const b = DV.battery;
  const btn = `<button class="btn small ${b ? '' : 'primary'}" data-action="dvBattery" type="button" ${DV.batBusy ? 'disabled' : ''}>${b ? 'Read again' : 'Check battery'}</button>`;
  let body;
  if (DV.batBusy) body = `<p class="hint"><span class="spinner"></span>&nbsp; Reading the battery...</p>`;
  else if (!b) body = `<p class="hint">Shows how much capacity the battery has left compared to when it was new, and how many charge cycles it has had.</p>`;
  else if (b.error) body = `<p class="hint">${esc(b.error)}</p>`;
  else if (!b.present) body = `<p class="hint">No battery found. This looks like a desktop PC.</p>`;
  else {
    const kind = { ok: 'ok', warn: 'warn', bad: 'bad' }[b.grade] || 'info';
    const mwh = v => v ? Math.round(v).toLocaleString() + ' mWh' : '-';
    body = `<div class="hero" style="gap:28px;flex-wrap:wrap">${b.health ? ring(b.health, 150, 12, kind === 'ok' ? 'var(--ok)' : kind === 'warn' ? 'var(--warn)' : 'var(--bad)', b.health + '%') : ''}
      <div class="grow stack" style="gap:6px;min-width:260px"><div>${badge(kind, b.verdict, true)}</div>
      ${kv('Capacity when new', mwh(b.design_mwh))}${kv('Capacity now (full charge)', mwh(b.full_mwh))}${kv('Charge cycles', b.cycles ? String(b.cycles) : 'Not reported')}
      ${kv('Charge level', b.charge >= 0 ? b.charge + '%' + (b.charging ? ' (charging)' : '') : '-')}</div></div>
      ${b.health && b.health < 70 ? '<p class="hint" style="margin-top:12px">Below 70% the battery usually needs replacing. For a detailed Windows report, use Toolkit, Reports, Battery report.</p>' : ''}${b.demo ? '<p class="hint">Sample data.</p>' : ''}`;
  }
  return `<div class="card"><div class="card-head"><h2>Battery health</h2>${btn}</div>${body}</div>`;
}

function dvDrives() {
  const d = DV.drives;
  const btn = `<button class="btn small ${d ? '' : 'primary'}" data-action="dvScan" type="button" ${DV.listBusy || DV.busy ? 'disabled' : ''}>${d ? 'Scan again' : 'Find USB drives'}</button>`;
  let body;
  if (DV.listBusy) body = `<p class="hint"><span class="spinner"></span>&nbsp; Looking for USB drives...</p>`;
  else if (!d) body = `<p class="hint">Finds flash drives and USB disks, checks their health, tests reading and writing, and can format them. Only removable USB drives are ever listed.</p>`;
  else if (d.error) body = `<p class="hint">${esc(d.error)}</p>`;
  else if (!d.drives.length) body = `<p class="hint">No USB drive found. Plug one in and press Scan again.</p>`;
  else body = d.drives.map(dr => {
    const vols = dr.volumes.length ? dr.volumes.map(v => `<div class="item"><div class="grow"><span class="title">${esc(v.letter)}: ${esc(v.label || 'No label')}</span>
      <span class="sub">${esc(v.fs || 'Unknown file system')} · ${dvFmt(v.size_gb)} total · ${dvFmt(v.free_gb)} free</span></div>
      <button class="btn small" data-action="dvOpen" data-id="${esc(v.letter)}" type="button">Open</button>
      <button class="btn small" data-action="dvCheck" data-id="${esc(v.letter)}" type="button" ${DV.busy ? 'disabled' : ''}>Check</button>
      <button class="btn small danger" data-action="dvFormat" data-id="${esc(v.letter)}" data-disk="${dr.disk}" type="button" ${DV.busy ? 'disabled' : ''}>Format</button></div>`).join('')
      : `<p class="hint" style="margin:8px 0">No usable partition (no drive letter). The disk may be blank or use a file system Windows cannot read.</p>`;
    const hk = /healthy/i.test(dr.health) ? 'ok' : dr.health ? 'warn' : 'info';
    return `<div class="rp-fix" style="margin-bottom:12px"><div class="top"><b>${esc(dr.name || 'USB drive')} · ${dvFmt(dr.size_gb)}</b>${badge(hk, dr.health || 'Unknown', true)}</div>
      <span class="hint">Partition style: ${esc(dr.style || 'unknown')}</span>${vols}</div>`;
  }).join('') + (d.demo ? '<p class="hint">Sample data.</p>' : '');
  return `<div class="card"><div class="card-head"><h2>USB flash drives</h2>${btn}</div>${body}
    <p class="hint" style="margin-top:12px">Check reads the file system without changing it, then writes a 64 MB test file on the drive, reads it back and compares it. A drive that returns different data is not reliable. USB drives do not report SMART data, so this is the most Windows can tell.</p></div>`;
}

ACTIONS.dvBattery = async () => { DV.batBusy = true; dvPaint(); DV.battery = await dvCall('battery_info'); DV.batBusy = false; dvPaint(); };
ACTIONS.dvScan = async () => { DV.listBusy = true; dvPaint(); DV.drives = await dvCall('usb_list'); DV.listBusy = false; dvPaint(); };
ACTIONS.dvOpen = el => dvCall('usb_open', el.dataset.id);
ACTIONS.dvCheck = async el => {
  const L = el.dataset.id; if (DV.busy) return;
  if (!(await confirmBox({ title: `Check drive ${L}:`, ok: 'Start check', body: '<p>Scans the file system, then writes and reads back a 64 MB test file on the drive. Your files are not changed. Keep the drive plugged in. It can take a few minutes.</p>' }))) return;
  DV.busy = `Checking drive ${L}:...`; dvPaint();
  const r = await dvCall('usb_check', L);
  DV.busy = ''; dvPaint();
  infoBox(r.ok ? `Drive ${L}: check finished` : `Drive ${L}: check finished with errors`, `<pre>${esc(r.output || '(no output)')}</pre>`);
};

/* Format asks for the drive letter to be typed, so a misclick cannot erase a drive. */
function dvFormatDialog(drive, vol) {
  return new Promise(resolve => {
    const back = document.createElement('div'); back.className = 'modal-back';
    const L = vol.letter;
    back.innerHTML = `<div class="modal" role="dialog" aria-modal="true"><h2>Format drive ${esc(L)}:</h2><div class="body">
      <p><b>Everything on this drive will be erased and cannot be recovered.</b></p>
      <p class="hint">${esc(drive.name)} · ${dvFmt(drive.size_gb)} · now ${esc(vol.fs || 'unknown')}, ${dvFmt(Math.round((vol.size_gb - vol.free_gb) * 10) / 10)} used</p>
      <label class="rp-opt"><span class="grow"><span>File system</span></span><select id="dvFs"><option value="exFAT">exFAT (large files, Windows and Mac)</option><option value="FAT32">FAT32 (works everywhere, files under 4 GB)</option><option value="NTFS">NTFS (Windows only)</option></select></label>
      <label class="rp-opt"><span class="grow"><span>Label</span></span><input id="dvLabel" type="text" maxlength="32" value="${esc(vol.label || '')}" style="width:180px"></label>
      <label class="rp-opt"><input id="dvFull" type="checkbox"><span class="grow"><span>Full format</span><span class="sub">Slow (can take hours) but also checks every sector for errors. Quick is enough for most cases.</span></span></label>
      <p style="margin-top:12px">Type <b>${esc(L)}</b> to confirm:</p><input id="dvConfirm" type="text" maxlength="2" autocomplete="off" style="width:60px;text-transform:uppercase"></div>
      <div class="acts"><button class="btn" data-v="0" type="button">Cancel</button><button class="btn danger" data-v="1" id="dvGo" type="button" disabled>Format</button></div></div>`;
    const done = v => { back.remove(); document.removeEventListener('keydown', onKey); resolve(v); };
    const onKey = e => { if (e.key === 'Escape') done(null); };
    back.addEventListener('click', e => {
      const b = e.target.closest('[data-v]');
      if (b) done(b.dataset.v === '1' ? { fs: back.querySelector('#dvFs').value, label: back.querySelector('#dvLabel').value, full: back.querySelector('#dvFull').checked } : null);
      else if (e.target === back) done(null);
    });
    back.addEventListener('input', () => { back.querySelector('#dvGo').disabled = back.querySelector('#dvConfirm').value.trim().toUpperCase() !== L.toUpperCase(); });
    document.addEventListener('keydown', onKey);
    document.body.appendChild(back); back.querySelector('#dvConfirm').focus();
  });
}
ACTIONS.dvFormat = async el => {
  if (DV.busy) return;
  const dr = (DV.drives.drives || []).find(x => String(x.disk) === el.dataset.disk), vol = dr && dr.volumes.find(v => v.letter === el.dataset.id); if (!vol) return;
  const o = await dvFormatDialog(dr, vol); if (!o) return;
  DV.busy = `Formatting drive ${vol.letter}:...`; dvPaint();
  const r = await dvCall('usb_format', vol.letter, o.fs, o.label, o.full);
  DV.busy = ''; DV.drives = await dvCall('usb_list'); dvPaint();
  if (r.ok) toast(`Drive ${vol.letter}: formatted as ${o.fs}`);
  else infoBox('Format failed', `<pre>${esc(r.output || r.error || 'Unknown error')}</pre><p class="hint">FAT32 is not offered by Windows for drives above 32 GB. Choose exFAT or NTFS for those. A drive in use by another program cannot be formatted either.</p>`);
};
