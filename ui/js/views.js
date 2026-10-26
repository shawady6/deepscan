/* DeepScan screens. Each view: { title, render() -> html, after(el)? } */

function section(id, build) {
  if (DS.data[id]) return build(DS.data[id]);
  if (DS.errors[id]) return errorCard(id, DS.errors[id]);
  return skeleton(140);
}
function updateChip() {
  const hw = DS.data.hardware; if (!hw || !hw.device) return;
  const n = $('#chipName'), s = $('#chipSub');
  if (n) n.textContent = hw.device.name || 'This device';
  if (s) s.textContent = hw.demo ? 'Demo data' : (hw.os.name + (hw.os.version ? ' ' + hw.os.version : ''));
}
const FIND_BADGE = { bad: ['bad', 'Critical'], warn: ['warn', 'Warning'], info: ['info', 'Info'], ok: ['ok', 'OK'] };

ACTIONS.scan = () => runScan();

/* ============================ Overview ============================ */
VIEWS.overview = {
  title: 'Overview',
  render() {
    const d = DS.data, hw = d.hardware;
    const head = pageHead('System overview', DS.lastScan ? `Last full scan at ${fmt.time(DS.lastScan)}` : 'Run a scan to see how this PC is doing',
      `<button class="btn" data-go="reports" type="button">Share specs</button><button class="btn primary" data-action="scan" type="button">Run full scan</button>`);
    if (!Object.keys(d).length && !Object.keys(DS.errors).length && !DS.scan.running) {
      return head + `<div class="card empty"><h2>No scan yet</h2><p class="hint">DeepScan checks hardware, storage, network, security and software in about a minute.</p><button class="btn primary" data-action="scan" type="button">Run first scan</button></div>`;
    }
    const score = healthScore(), f = findings();
    const nb = f.filter(x => x.kind === 'bad').length, nw = f.filter(x => x.kind === 'warn').length;
    const summary = nb || nw ? `${nb ? nb + ' critical issue' + (nb > 1 ? 's' : '') : ''}${nb && nw ? ' and ' : ''}${nw ? nw + ' warning' + (nw > 1 ? 's' : '') : ''} found.` : 'Nothing needs attention.';
    const scoreCard = `<div class="card hero grow" style="flex:1 1 340px">${ring(score, 140, 12, scoreColor(score))}<div class="stack" style="gap:6px"><span class="card-note">Health score</span><h2>${healthWord(score)}</h2><p class="hint">${summary}</p></div></div>`;
    const vol = systemVolume(), L = DS.live || {};
    const cpuName = hw && hw.cpu ? hw.cpu.name : '';
    const kCpu = `<div class="card kpi"><span class="label">Processor</span><span class="value"><span data-live="cpu">${L.cpu ?? '-'}</span><small>%</small></span>${bar(L.cpu || 0, '').replace('<i ', '<i data-livebar="cpu" ')}<span class="sub">${esc(cpuName || ' ')}</span></div>`;
    const kMem = `<div class="card kpi"><span class="label">Memory</span><span class="value"><span data-live="mem">${L.mem ?? '-'}</span><small>%</small></span>${bar(L.mem || 0, '').replace('<i ', '<i data-livebar="mem" ')}<span class="sub" data-live="mem_sub">${hw && hw.memory ? hw.memory.total_gb + ' GB installed' : ' '}</span></div>`;
    let kSto = skeleton(120);
    if (vol) { const used = vol.size_gb - vol.free_gb, p = Math.round(used / vol.size_gb * 100); kSto = `<div class="card kpi"><span class="label">Storage (${esc(vol.letter)}:)</span><span class="value">${p}<small>%</small></span>${bar(p, pctKind(p, 75, 90))}<span class="sub">${Math.round(used)} of ${Math.round(vol.size_gb)} GB used</span></div>`; }
    const temp = hw && hw.sensors && hw.sensors[0];
    const kFour = temp
      ? `<div class="card kpi"><span class="label">Temperature</span><span class="value">${Math.round(temp.value)}<small> °C</small></span>${bar(Math.min(100, temp.value), pctKind(temp.value, 70, 85))}<span class="sub">${esc(temp.name)}</span></div>`
      : `<div class="card kpi"><span class="label">Network now</span><span class="value"><span data-live="down">${L.down_mbps ?? 0}</span><small> Mbps down</small></span><span class="sub">Up <span data-live="up">${L.up_mbps ?? 0}</span> Mbps</span></div>`;
    const top = `<div class="row">${scoreCard}<div class="grow2"><div class="kpis">${kCpu}${kMem}${kSto}${kFour}</div></div></div>`;
    const rows = f.map(x => { const [k, t] = FIND_BADGE[x.kind]; return `<div class="item">${badge(k, t, true)}<div class="grow"><span class="title">${esc(x.title)}</span>${x.sub ? `<span class="sub">${esc(x.sub)}</span>` : ''}</div>${x.go ? `<button class="link" data-go="${x.go}" type="button">View</button>` : ''}</div>`; }).join('');
    const findCard = `<div class="card grow2"><h2>Findings</h2>${rows || '<div class="sk" style="height:90px"></div>'}</div>`;
    const net = d.network && d.network.active;
    const sum = hw ? `<div class="card grow3"><h2>System summary</h2>${kv('Device', [hw.device.maker, hw.device.model].filter(Boolean).join(' '))}${kv('Windows', hw.os.name + ' ' + hw.os.version)}${kv('Graphics', hw.gpu[0] && hw.gpu[0].name)}${kv('Memory', hw.memory.total_gb + ' GB')}${kvRaw('Uptime', `<span class="mono" data-live="uptime">${L.uptime_s ? fmt.uptime(L.uptime_s) : '-'}</span>`)}${kv('IP address', net && net.ipv4)}</div>` : `<div class="grow3">${skeleton(260)}</div>`;
    return head + top + `<div class="row">${findCard}${sum}</div>`;
  },
  after() { updateChip(); applyLive(); },
};

/* ============================= Scanning ============================ */
function stepIcon(st) {
  if (st === 'done') return `<svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="9" fill="var(--accent)"/><path d="M6 10.5l2.8 2.8L14 8" stroke="var(--accent-ink)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  if (st === 'running') return '<span class="spinner" style="width:20px;height:20px"></span>';
  if (st === 'failed') return `<svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="9" fill="var(--bad-fg)"/><path d="M10 5.5v5.5M10 14v.5" stroke="var(--bg)" stroke-width="2" stroke-linecap="round"/></svg>`;
  return `<svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="8" stroke="var(--line2)" stroke-width="2"/></svg>`;
}
const STEP_TEXT = { queued: 'Queued', running: 'Running', done: 'Done', failed: 'Failed' };
function scanTitle() {
  const s = DS.scan, i = STEPS.findIndex(x => s.steps[x[0]] === 'running');
  if (s.running && i >= 0) return [`Step ${i + 1} of ${STEPS.length}`, STEPS[i][1], 'You can keep working while the scan runs.'];
  return ['Finished', 'Scan complete', 'Opening your results...'];
}
VIEWS.scan = {
  title: 'Scanning',
  render() {
    const s = DS.scan, [a, b, c] = scanTitle();
    const steps = STEPS.map(([id, label]) => { const st = s.steps[id] || 'queued'; return `<div class="step ${st}" data-step="${id}"><span class="ico">${stepIcon(st)}</span><span class="name">${esc(label)}</span><span class="state">${STEP_TEXT[st]}</span></div>`; }).join('');
    return `<div class="card scan-card" id="scanRoot"><div class="hero"><div id="scanRing">${ring(s.progress, 200, 14, 'var(--accent)', s.progress + '%')}</div>
      <div class="stack" style="flex:1 1 240px;gap:8px"><span class="card-note" id="scanStep">${a}</span><h1 style="font-size:26px;font-weight:600" id="scanTitle">${esc(b)}</h1><p class="hint" id="scanSub">${c}</p></div></div>
      <div>${steps}</div><div class="log" id="scanLog" aria-live="off">${s.log.map(l => `<div>${esc(l)}</div>`).join('')}</div>
      <div class="actions" style="justify-content:flex-end"><button class="btn" data-go="overview" type="button">Hide and keep working</button></div></div>`;
  },
  update() {
    const s = DS.scan, fg = $('#scanRing .fg'); if (!fg) return;
    fg.dataset.pct = s.progress; fg.style.strokeDashoffset = (+fg.dataset.c) * (1 - s.progress / 100);
    $('#scanRing text').textContent = s.progress + '%';
    const [a, b, c] = scanTitle(); $('#scanStep').textContent = a; $('#scanTitle').textContent = b; $('#scanSub').textContent = c;
    $$('[data-step]').forEach(el => { const st = s.steps[el.dataset.step] || 'queued'; el.className = 'step ' + st; el.querySelector('.ico').innerHTML = stepIcon(st); el.querySelector('.state').textContent = STEP_TEXT[st]; });
    const log = $('#scanLog'); log.innerHTML = s.log.map(l => `<div>${esc(l)}</div>`).join(''); log.scrollTop = log.scrollHeight;
  },
};

/* ============================= Hardware ============================ */
VIEWS.hardware = {
  title: 'Hardware',
  render() {
    const hw = DS.data.hardware;
    const head = pageHead('Hardware', hw ? esc(hw.cpu.name) : 'Reading components...');
    if (!hw) return head + (DS.errors.hardware ? errorCard('hardware', DS.errors.hardware) : skeleton(300));
    const cpu = `<div class="card grow"><h2>Processor</h2>${kv('Model', hw.cpu.name)}${kv('Cores / threads', `${hw.cpu.cores} / ${hw.cpu.threads}`)}${kv('Max clock', hw.cpu.max_ghz ? hw.cpu.max_ghz + ' GHz' : '')}${kv('Socket', hw.cpu.socket)}${kv('L3 cache', hw.cpu.l3_mb ? hw.cpu.l3_mb + ' MB' : '')}</div>`;
    const gpus = hw.gpu.map(g => `<div class="card grow"><h2>Graphics</h2>${kv('Adapter', g.name)}${kv('Video memory', g.vram)}${kv('Driver date', g.driver_date)}${kv('Display', g.display)}</div>`).join('');
    const board = `<div class="card grow"><h2>Motherboard and firmware</h2>${kv('Board', [hw.board.maker, hw.board.model].filter(Boolean).join(' '))}${kv('BIOS version', hw.board.bios_version)}${kv('BIOS date', hw.board.bios_date)}${kv('TPM', hw.tpm)}${kv('Secure Boot', hw.secure_boot)}</div>`;
    const mods = hw.memory.modules.map(m => `<tr><td class="mono">${esc(m.slot)}</td><td class="mono">${m.size_gb} GB</td><td class="mono">${esc(m.type)}${m.speed ? ' ' + m.speed : ''}</td><td>${esc(m.maker)}</td></tr>`).join('');
    const empty = hw.memory.slots_total ? Math.max(0, hw.memory.slots_total - hw.memory.modules.length) : 0;
    const mem = `<div class="card grow"><div class="card-head"><h2>Memory</h2><span class="card-note">${hw.memory.total_gb} GB total${hw.memory.slots_total ? ` · ${hw.memory.modules.length} of ${hw.memory.slots_total} slots used` : ''}</span></div><div class="tw"><table class="t"><thead><tr><th>Slot</th><th>Size</th><th>Type</th><th>Maker</th></tr></thead><tbody>${mods}</tbody></table></div>${empty ? `<p class="hint" style="margin-top:12px">${empty} empty slot${empty > 1 ? 's' : ''} available for upgrades.</p>` : ''}</div>`;
    const sys = `<div class="card grow"><h2>Windows and device</h2>${kv('Windows', hw.os.name + ' ' + hw.os.version)}${kv('Build', hw.os.build + ' · ' + hw.os.arch)}${kv('Installed on', hw.os.install_date)}${kv('Computer', hw.device.name)}${kv('Manufacturer', [hw.device.maker, hw.device.model].filter(Boolean).join(' '))}${kv('Domain / workgroup', hw.device.domain)}</div>`;
    const sens = `<div class="card grow"><h2>Temperatures</h2>${hw.sensors.length ? hw.sensors.map(s => kv(s.name, s.value + ' ' + s.unit)).join('') : '<p class="hint">Windows does not share CPU and GPU temperatures without a sensor driver. Support for that is planned.</p>'}</div>`;
    return head + `<div class="row">${cpu}${gpus}</div><div class="row">${board}${mem}</div><div class="row">${sys}${sens}</div>`;
  },
  after() { updateChip(); },
};

/* ============================== Storage ============================ */
const encBadge = e => e === 'on' ? badge('ok', 'BitLocker on') : e === 'off' ? badge('warn', 'BitLocker off') : '<span class="muted">Not checked</span>';
VIEWS.storage = {
  title: 'Storage',
  render() {
    const st = DS.data.storage;
    const need = st ? st.disks.filter(d => d.health !== 'ok').length : 0;
    const head = pageHead('Storage health', st ? `${st.disks.length} drive${st.disks.length === 1 ? '' : 's'} checked${need ? ` · ${need} need${need > 1 ? '' : 's'} attention` : ' · all healthy'}` : 'Reading drives...',
      `<button class="btn" data-rescan="storage" type="button">Re-check</button>`);
    if (!st) return head + (DS.errors.storage ? errorCard('storage', DS.errors.storage) : skeleton(300));
    const cards = st.disks.map(d => {
      const vols = st.volumes.filter(v => d.volumes.includes(v.letter)).map(v => { const used = v.size_gb - v.free_gb, p = Math.round(used / v.size_gb * 100); return `<div class="stack" style="gap:8px"><div class="kv" style="border:0;padding:0"><span>Space used (${esc(v.letter)}:)</span><span class="mono">${Math.round(used)} of ${Math.round(v.size_gb)} GB</span></div>${bar(p, pctKind(p, 75, 90), true)}</div>`; }).join('');
      const life = d.wear_pct != null ? `<div class="stack" style="gap:8px"><div class="kv" style="border:0;padding:0"><span>Life remaining</span><span class="mono">${100 - d.wear_pct}%</span></div>${bar(100 - d.wear_pct, 100 - d.wear_pct < 20 ? 'bad' : 100 - d.wear_pct < 40 ? 'warn' : '', true)}</div>` : '';
      const note = d.health !== 'ok' ? `<div class="alert" style="padding:14px 16px"><span>${d.health === 'unknown' ? 'Health could not be read. Run as administrator.' : `Windows reports this drive as "${esc(d.status)}"${d.read_errors ? ` with ${d.read_errors} read errors` : ''}. Back up important data soon.`}</span></div>` : '';
      return `<div class="card grow stack"><div class="card-head"><div class="stack" style="gap:2px"><h2 style="margin:0">${esc(d.name)}</h2><span class="card-note">Disk ${esc(d.id)} · ${esc(d.type)} · ${d.size_gb} GB${d.volumes.length ? ' · ' + d.volumes.map(v => v + ':').join(' ') : ''}</span></div>${statusBadge(d.health === 'unknown' ? 'unknown' : d.health, false)}</div>${life}${vols}
        <div class="row" style="gap:24px"><div class="stack" style="gap:2px"><span class="card-note">Temperature</span><span class="mono">${d.temp_c != null ? d.temp_c + ' °C' : '-'}</span></div><div class="stack" style="gap:2px"><span class="card-note">Power-on hours</span><span class="mono">${fmt.hours(d.power_on_hours)}</span></div><div class="stack" style="gap:2px"><span class="card-note">Read errors</span><span class="mono">${d.read_errors ?? '-'}</span></div></div>${note}</div>`;
    }).join('');
    const rows = st.volumes.map(v => `<tr><td><b>${esc(v.letter)}:</b> ${esc(v.label)}</td><td>${v.disk == null ? 'Removable' : 'Disk ' + esc(v.disk)}</td><td class="mono">${esc(v.fs)}</td><td class="num mono">${v.size_gb} GB</td><td class="num mono">${v.free_gb} GB</td><td class="num">${encBadge(v.encryption)}</td></tr>`).join('');
    const table = `<div class="card"><h2>Volumes</h2><div class="tw"><table class="t" style="min-width:640px"><thead><tr><th>Volume</th><th>Disk</th><th>File system</th><th class="num">Size</th><th class="num">Free</th><th class="num">Encryption</th></tr></thead><tbody>${rows}</tbody></table></div></div>`;
    const temp = `<div class="card"><h2>Temporary files</h2>${kv('Windows temp folder', fmt.mb(st.temp_mb))}<p class="hint" style="margin-top:12px">Temporary files can usually be deleted safely. Cleanup tools are planned for a later version.</p></div>`;
    return head + `<div class="row">${cards}</div>${table}${temp}`;
  },
};

/* ============================== Software =========================== */
DS.sw = { q: '', key: 'name', dir: 1 };
function progRows() {
  const sw = DS.data.software; if (!sw) return '';
  const q = DS.sw.q.toLowerCase(), { key, dir } = DS.sw;
  let list = sw.programs.filter(p => !q || (p.name + ' ' + p.publisher).toLowerCase().includes(q));
  list = list.slice().sort((a, b) => { const x = a[key] ?? '', y = b[key] ?? ''; return (typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y), undefined, { numeric: true })) * dir; });
  const shown = list.slice(0, 200);
  $('#progCount') && ($('#progCount').textContent = `Showing ${shown.length} of ${sw.programs.length} programs`);
  return shown.map(p => `<tr><td><b style="font-weight:500">${esc(p.name)}</b></td><td>${esc(p.publisher)}</td><td class="mono">${esc(p.version)}</td><td class="num mono">${p.size_mb ? fmt.mb(p.size_mb) : '-'}</td><td class="mono">${esc(p.installed || '-')}</td></tr>`).join('') || '<tr><td colspan="5" class="muted">No programs match your search.</td></tr>';
}
const sortTh = (key, label, cls = '') => `<th class="sort ${cls}" data-action="swSort" data-key="${key}" tabindex="0" aria-sort="${DS.sw.key === key ? (DS.sw.dir > 0 ? 'ascending' : 'descending') : 'none'}">${label}${DS.sw.key === key ? (DS.sw.dir > 0 ? ' ↑' : ' ↓') : ''}</th>`;
ACTIONS.swSearch = el => { DS.sw.q = el.value; $('#progBody').innerHTML = progRows(); };
ACTIONS.swSort = el => { const k = el.dataset.key; DS.sw.dir = DS.sw.key === k ? -DS.sw.dir : 1; DS.sw.key = k; refresh(); };
VIEWS.software = {
  title: 'Software',
  render() {
    const sw = DS.data.software;
    const head = pageHead('Software', sw ? `${sw.programs.length} programs · ${sw.startup.filter(s => s.enabled).length} start with Windows` : 'Reading installed software...', `<button class="btn" data-rescan="software" type="button">Re-check</button>`);
    if (!sw) return head + (DS.errors.software ? errorCard('software', DS.errors.software) : skeleton(300));
    const kp = `<div class="kpis"><div class="card kpi"><span class="label">Installed programs</span><span class="value">${sw.programs.length}</span></div><div class="card kpi"><span class="label">Start with Windows</span><span class="value">${sw.startup.filter(s => s.enabled).length}</span><span class="sub">${sw.startup.filter(s => !s.enabled).length} more are turned off</span></div><div class="card kpi"><span class="label">Running processes</span><span class="value">${sw.processes.length}<small> top</small></span></div></div>`;
    const prog = `<div class="card"><div class="card-head" style="flex-wrap:wrap"><h2>Installed programs</h2><input class="search" type="search" placeholder="Search programs" aria-label="Search programs" value="${esc(DS.sw.q)}" data-input="swSearch"></div>
      <div class="tw"><table class="t" style="min-width:680px"><thead><tr>${sortTh('name', 'Name')}${sortTh('publisher', 'Publisher')}${sortTh('version', 'Version')}${sortTh('size_mb', 'Size', 'num')}${sortTh('installed', 'Installed')}</tr></thead><tbody id="progBody">${progRows()}</tbody></table></div><p class="card-note" id="progCount" style="margin-top:12px"></p></div>`;
    const start = `<div class="card grow"><div class="card-head"><h2>Startup programs</h2><span class="card-note">Read only</span></div>${sw.startup.length ? sw.startup.map(s => `<div class="item"><div class="grow"><span class="title">${esc(s.name)}</span><span class="sub">${esc(s.location)} · ${esc(s.command)}</span></div>${badge(s.enabled ? 'ok' : 'info', s.enabled ? 'On' : 'Off', true)}</div>`).join('') : '<p class="hint">No startup items found.</p>'}<p class="hint" style="margin-top:12px">To turn items on or off use Task Manager > Startup apps.</p></div>`;
    const procs = `<div class="card grow"><h2>Running now · top processes</h2><div class="tw"><table class="t"><thead><tr><th>Process</th><th class="num">CPU</th><th class="num">Memory</th></tr></thead><tbody>${sw.processes.map(p => `<tr><td>${esc(p.name)}</td><td class="num mono">${p.cpu}%</td><td class="num mono">${fmt.mb(p.mem_mb)}</td></tr>`).join('')}</tbody></table></div></div>`;
    return head + kp + prog + `<div class="row">${start}${procs}</div>`;
  },
  after() { progRows(); },
};

/* =============================== Network =========================== */
function speedAvg() {
  const h = DS.speed.history.slice(-7); if (!h.length) return null;
  const a = k => { const v = h.map(x => x[k]).filter(x => x != null); return v.length ? v.reduce((s, x) => s + x, 0) / v.length : null; };
  return { n: h.length, down: a('down'), up: a('up'), ping: a('ping'), jitter: a('jitter'), h };
}
ACTIONS.speedtest = async () => {
  if (DS.speed.running) return;
  DS.speed.running = true; refresh();
  let r; try { r = await api('run_speedtest'); } catch (e) { r = { ok: false, error: String(e) }; }
  DS.speed.running = false;
  if (r.ok) { DS.speed.history = r.history; toast(`Download ${r.down} Mbps · Upload ${r.up} Mbps`); } else toast(r.error || 'Speed test failed');
  if (DS.route === 'network') refresh();
};
ACTIONS.publicIp = async el => {
  el.disabled = true; el.textContent = '...';
  const r = await api('public_ip'); DS.publicIp = r.ok ? r.ip : null;
  if (DS.route === 'network') refresh(); if (!r.ok) toast(r.error);
};
ACTIONS.copyNet = () => {
  const n = DS.data.network; if (!n || !n.active) return;
  const a = n.active, t = [`Computer: ${n.hostname}`, `Connection: ${a.type} (${a.name})`, `IPv4: ${a.ipv4}`, `Subnet mask: ${a.mask}`, `Gateway: ${a.gateway}`, `DNS: ${a.dns.join(', ')}`, `MAC: ${a.mac}`].join('\n');
  navigator.clipboard.writeText(t).then(() => toast('Network details copied'), () => toast('Could not copy'));
};
const pingStatus = p => p.ms == null ? statusBadge('bad', false).replace('Critical', 'No reply') : p.loss > 0 ? badge('warn', 'Packet loss') : p.ms > 100 ? badge('warn', 'Slow') : badge('ok', 'OK');
VIEWS.network = {
  title: 'Network',
  render() {
    const n = DS.data.network;
    const head = pageHead('Network', n ? (n.active ? `Connected through ${esc(n.active.type)}` : 'No active connection') : 'Reading network...',
      `<button class="btn" data-action="copyNet" type="button">Copy network details</button><button class="btn primary" data-action="speedtest" type="button" ${DS.speed.running ? 'disabled' : ''}>${DS.speed.running ? '<span class="spinner"></span> Testing...' : 'Run speed test'}</button>`);
    if (!n) return head + (DS.errors.network ? errorCard('network', DS.errors.network) : skeleton(300));
    const a = n.active, w = n.wifi;
    const conn = a ? `<div class="card grow"><div class="card-head"><h2>Active connection</h2>${badge('ok', 'Connected')}</div>${kv('Type', a.type)}${w ? kv('Network name (SSID)', w.ssid) : ''}${w && w.signal != null ? kv('Signal', w.signal + '%') : ''}${w ? kv('Radio and channel', [w.radio, w.channel && 'channel ' + w.channel].filter(Boolean).join(' · ')) : ''}${kv('Link speed', a.speed)}${w ? kv('Security', w.security) : ''}${kv('Adapter', a.desc)}${kv('MAC address', a.mac)}</div>` : `<div class="card grow"><h2>Active connection</h2><p class="hint">This PC is not connected to a network.</p></div>`;
    const pub = DS.publicIp ? `<span class="mono">${esc(DS.publicIp)}</span>` : `<button class="btn small" data-action="publicIp" type="button">Show</button>`;
    const addr = a ? `<div class="card grow"><h2>IP and addressing</h2>${kv('Local IPv4', a.ipv4)}${kv('Subnet mask', a.mask)}${kv('Default gateway', a.gateway)}${kv('DNS servers', a.dns.join(' · '))}${kv('DHCP', a.dhcp)}${kv('IPv6', a.ipv6)}${kvRaw('Public IP', pub)}</div>` : '';
    const avg = speedAvg();
    let speed;
    if (!avg) speed = `<div class="card"><h2>Speed</h2><p class="hint">No speed test yet. Press "Run speed test" to measure download, upload and ping. The test uses some data and takes about 15 seconds.</p></div>`;
    else {
      const maxV = Math.max(...avg.h.map(x => Math.max(x.down, x.up)), 1);
      const bars = avg.h.map(x => `<div style="flex:1;display:flex;align-items:flex-end;justify-content:center;gap:4px" title="${esc(x.time)}: ${x.down} down, ${x.up} up Mbps"><i class="vb" data-h="${Math.round(x.down / maxV * 150)}" style="background:var(--accent)"></i><i class="vb" data-h="${Math.round(x.up / maxV * 150)}" style="background:var(--info-fg)"></i></div>`).join('');
      const labels = avg.h.map(x => `<span style="flex:1;text-align:center">${esc((x.time || '').slice(5, 10))}</span>`).join('');
      const k = (l, v, u, c) => `<div class="stack" style="gap:4px;flex:1 1 140px"><span class="card-note">${l}</span><span class="mono" style="font-size:28px;color:${c || 'var(--text)'}">${v == null ? '-' : Math.round(v)} <small class="muted" style="font-size:14px;font-family:var(--font)">${u}</small></span></div>`;
      speed = `<div class="card stack" style="gap:20px"><div class="card-head"><h2>Speed · average of last ${avg.n} test${avg.n > 1 ? 's' : ''}</h2><span class="card-note">Last: ${esc(avg.h[avg.h.length - 1].time)}</span></div>
        <div class="row" style="gap:16px">${k('Download', avg.down, 'Mbps', 'var(--accent)')}${k('Upload', avg.up, 'Mbps', 'var(--info-fg)')}${k('Ping', avg.ping, 'ms')}${k('Jitter', avg.jitter, 'ms')}</div>
        <div role="img" aria-label="Download and upload speed of the last tests" style="display:flex;align-items:flex-end;gap:12px;height:170px;border-bottom:1px solid var(--line2)">${bars}</div>
        <div style="display:flex;gap:12px;font-size:12px;color:var(--muted)">${labels}</div>
        <div style="display:flex;gap:20px;font-size:13px"><span><i class="dot" style="background:var(--accent)"></i> Download</span><span><i class="dot" style="background:var(--info-fg)"></i> Upload</span></div></div>`;
    }
    const pr = n.ping.map(p => `<tr><td>${esc(p.label)} <span class="muted mono">${esc(p.target)}</span></td><td class="num mono">${p.ms == null ? '-' : p.ms + ' ms'}</td><td class="num mono">${p.loss}%</td><td class="num">${pingStatus(p)}</td></tr>`).join('') + (n.dns_ms != null ? `<tr><td>DNS lookup <span class="muted mono">google.com</span></td><td class="num mono">${n.dns_ms} ms</td><td class="num mono">-</td><td class="num">${n.dns_ms > 200 ? badge('warn', 'Slow') : badge('ok', 'OK')}</td></tr>` : '');
    const lat = `<div class="card grow"><h2>Latency and reachability</h2><div class="tw"><table class="t" style="min-width:380px"><thead><tr><th>Target</th><th class="num">Ping</th><th class="num">Loss</th><th class="num">Status</th></tr></thead><tbody>${pr}</tbody></table></div></div>`;
    const ad = `<div class="card grow"><h2>Network adapters</h2><div class="tw"><table class="t" style="min-width:380px"><thead><tr><th>Adapter</th><th>Speed</th><th class="num">Status</th></tr></thead><tbody>${n.adapters.map(x => `<tr><td>${esc(x.name)}<br><span class="card-note">${esc(x.desc)}</span></td><td class="mono">${esc(x.speed || '-')}</td><td class="num">${x.virtual && x.status === 'Up' ? badge('info', 'Virtual') : x.status === 'Up' ? badge('ok', 'Connected') : `<span class="muted">${esc(x.status)}</span>`}</td></tr>`).join('')}</tbody></table></div></div>`;
    return head + `<div class="row">${conn}${addr}</div>${speed}<div class="row">${lat}${ad}</div>`;
  },
};

/* =============================== Security ========================== */
VIEWS.security = {
  title: 'Security',
  render() {
    const s = DS.data.security;
    const head = pageHead('Security', s ? `${s.checks.length} checks` : 'Running checks...', `<button class="btn" data-rescan="security" type="button">Re-run checks</button>`);
    if (!s) return head + (DS.errors.security ? errorCard('security', DS.errors.security) : skeleton(300));
    const c = k => s.checks.filter(x => x.status === k).length;
    const score = s.score;
    const top = `<div class="row"><div class="card hero grow" style="flex:1 1 340px">${ring(score, 140, 12, scoreColor(score))}<div class="stack" style="gap:6px"><span class="card-note">Security score</span><h2>${healthWord(score)}</h2><p class="hint">${c('bad')} critical, ${c('warn')} warnings, ${c('unknown')} unknown.</p></div></div>
      <div class="grow2"><div class="kpis"><div class="card kpi"><span class="label">Passed</span><span class="value" style="color:var(--ok-fg)">${c('ok')}</span></div><div class="card kpi"><span class="label">Warnings</span><span class="value" style="color:var(--warn-fg)">${c('warn')}</span></div><div class="card kpi"><span class="label">Critical</span><span class="value" style="color:var(--bad-fg)">${c('bad')}</span></div></div></div></div>`;
    const worst = s.checks.find(x => x.status === 'bad');
    const alert = worst ? `<div class="alert"><div class="grow"><b>Fix first: ${esc(worst.name)}</b><span>${esc(worst.detail)}. ${esc(worst.advice)}</span></div></div>` : '';
    const groups = ['Protection', 'Accounts and access', 'Network and updates'].map(g => `<div class="card grow"><h2>${g}</h2>${s.checks.filter(x => x.group === g).map(x => `<div class="item">${statusBadge(x.status)}<div class="grow"><span class="title">${esc(x.name)}</span><span class="sub">${esc(x.detail)}</span>${x.advice && x.status !== 'ok' ? `<span class="sub" style="color:var(--text)">How to fix: ${esc(x.advice)}</span>` : ''}</div></div>`).join('')}</div>`).join('');
    const ports = `<div class="card"><h2>Listening ports</h2><div class="tw"><table class="t" style="min-width:420px"><thead><tr><th>Port</th><th>Process</th><th>Reachable from</th><th class="num">Status</th></tr></thead><tbody>${s.ports.map(p => `<tr><td class="mono">${p.port}</td><td>${esc(p.process)}</td><td>${esc(p.scope)}</td><td class="num">${p.flag === 'review' ? badge('warn', 'Review') : badge('ok', 'OK')}</td></tr>`).join('')}</tbody></table></div></div>`;
    return head + top + alert + `<div class="row">${groups}</div>` + ports;
  },
};
