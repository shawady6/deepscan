/* Reports: shareable spec card (drawn on a canvas) and a full HTML report. */

DS.card = { layout: 'landscape', theme: 'dark', score: true, board: true, device: false, ip: false, serial: false };

const CARD_PAL = {
  dark: { bg: '#0B1015', fg: '#E8ECF1', muted: '#9AA6B2', line: '#263039', accent: '#3DDC97', ok: ['#15332A', '#5BE3A8'], warn: ['#3A2F14', '#F2B84B'], bad: ['#3D1A1C', '#FF9A92'] },
  light: { bg: '#FFFFFF', fg: '#14202B', muted: '#53616F', line: '#DDE3E9', accent: '#0B8F5A', ok: ['#DDF5E9', '#0B6B43'], warn: ['#FCEFD0', '#7A4F00'], bad: ['#FBE0DE', '#A3271F'] },
};
const FONT = '"Segoe UI Variable Text","Segoe UI",system-ui,-apple-system,Roboto,sans-serif';
const marketing = g => { const v = Math.round(g * 1.0737); return v >= 1000 ? `${+(v / 1000).toFixed(1)} TB` : `${v} GB`; };

function specRows() {
  const hw = DS.data.hardware, st = DS.data.storage, net = DS.data.network, o = DS.card; if (!hw) return [];
  const g = hw.gpu[0], mods = hw.memory.modules, disks = (st && st.disks) || [];
  const rows = [
    ['Processor', `${hw.cpu.name}${hw.cpu.cores ? ` · ${hw.cpu.cores}C / ${hw.cpu.threads}T` : ''}`],
    ['Graphics', g ? `${g.name}${g.vram ? ' · ' + g.vram : ''}` : '-'],
    ['Memory', `${hw.memory.total_gb} GB${mods[0] ? ' ' + mods[0].type + (mods[0].speed ? ' ' + mods[0].speed : '') : ''}`],
    ['Storage', disks.length ? disks.map(d => `${marketing(d.size_gb)} ${d.type.replace(' SSD', '')}`).join(' + ') : '-'],
    ['System', `${hw.os.name} ${hw.os.version}`.trim()],
  ];
  if (o.board) rows.push(['Motherboard', [hw.board.maker.replace(/ INC\.?|COMPUTER/gi, '').trim(), hw.board.model].filter(Boolean).join(' ')]);
  if (o.ip && net && net.active) rows.push(['IP address', net.active.ipv4 || '-']);
  if (o.serial) rows.push(['Serial number', hw.device.serial || '-']);
  return rows;
}
function rr(ctx, x, y, w, h, r) { ctx.beginPath(); if (ctx.roundRect) ctx.roundRect(x, y, w, h, r); else ctx.rect(x, y, w, h); }
function fitText(ctx, text, maxW, size, weight = 500) {
  let s = size; ctx.font = `${weight} ${s}px ${FONT}`;
  while (ctx.measureText(text).width > maxW && s > 15) { s -= 1; ctx.font = `${weight} ${s}px ${FONT}`; }
  if (ctx.measureText(text).width > maxW) { while (text.length > 4 && ctx.measureText(text + '…').width > maxW) text = text.slice(0, -1); text += '…'; }
  return text;
}
function drawCard(canvas) {
  const o = DS.card, P = CARD_PAL[o.theme], rows = specRows(), hw = DS.data.hardware;
  const square = o.layout === 'square', W = square ? 1080 : 1200, pad = square ? 64 : 56, cols = square ? 1 : 2;
  const rowH = square ? 108 : 92, top = square ? 330 : 290, nrows = Math.ceil(rows.length / cols);
  const H = square ? 1080 : Math.max(630, top + nrows * rowH + 96);
  canvas.width = W; canvas.height = H;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = P.bg; rr(ctx, 0, 0, W, H, 0); ctx.fill();
  // logo
  const lx = pad, ly = pad;
  ctx.strokeStyle = P.accent; ctx.lineWidth = 3; rr(ctx, lx + 2, ly + 2, 40, 40, 11); ctx.stroke();
  ctx.beginPath(); ctx.arc(lx + 22, ly + 22, 8.5, 0, Math.PI * 2); ctx.stroke();
  ctx.lineCap = 'round'; [[22, 6, 22, 13], [22, 31, 22, 38], [6, 22, 13, 22], [31, 22, 38, 22]].forEach(([a, b, c, d]) => { ctx.beginPath(); ctx.moveTo(lx + a, ly + b); ctx.lineTo(lx + c, ly + d); ctx.stroke(); });
  ctx.fillStyle = P.fg; ctx.font = `700 30px ${FONT}`; ctx.textBaseline = 'middle'; ctx.textAlign = 'left'; ctx.fillText('DeepScan', lx + 58, ly + 23);
  // health pill
  if (o.score) {
    const sc = healthScore(), pal = sc >= 85 ? P.ok : sc >= 65 ? P.warn : P.bad, txt = `${sc}  ${healthWord(sc)}`;
    ctx.font = `600 22px ${FONT}`; const tw = ctx.measureText(txt).width, pw = tw + 40;
    ctx.fillStyle = pal[0]; rr(ctx, W - pad - pw, ly + 4, pw, 38, 19); ctx.fill();
    ctx.fillStyle = pal[1]; ctx.textAlign = 'center'; ctx.fillText(txt, W - pad - pw / 2, ly + 24); ctx.textAlign = 'left';
  }
  // title
  const title = o.device && hw.device.name ? hw.device.name : ([hw.device.maker, hw.device.model].filter(Boolean).join(' ') || 'My PC');
  ctx.fillStyle = P.muted; ctx.font = `500 20px ${FONT}`; ctx.fillText('SYSTEM SPECS', pad, top - 118);
  ctx.fillStyle = P.fg;
  ctx.fillText(fitText(ctx, title, W - pad * 2, 46, 700), pad, top - 68);
  // spec grid
  const colW = (W - pad * 2) / cols;
  rows.forEach(([label, value], i) => {
    const c = i % cols, r = Math.floor(i / cols), x = pad + c * colW, y = top + r * rowH;
    ctx.fillStyle = P.muted; ctx.font = `500 19px ${FONT}`; ctx.textBaseline = 'alphabetic'; ctx.fillText(label, x, y + 8);
    ctx.fillStyle = P.fg;
    ctx.fillText(fitText(ctx, value, colW - 28, 27, 600), x, y + 46);
  });
  // footer
  ctx.strokeStyle = P.line; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(pad, H - 74); ctx.lineTo(W - pad, H - 74); ctx.stroke();
  ctx.fillStyle = P.muted; ctx.font = `500 19px ${FONT}`; ctx.textBaseline = 'middle';
  ctx.fillText('Scanned with DeepScan', pad, H - 40);
  ctx.textAlign = 'right'; ctx.fillText(new Date().toISOString().slice(0, 10), W - pad, H - 40); ctx.textAlign = 'left';
}

function specMarkdown() {
  const rows = specRows(), hw = DS.data.hardware;
  const title = DS.card.device && hw.device.name ? hw.device.name : ([hw.device.maker, hw.device.model].filter(Boolean).join(' ') || 'My PC');
  return `**${title}**\n\n` + rows.map(([k, v]) => `- **${k}:** ${v}`).join('\n') + '\n\n_Scanned with DeepScan_';
}
const canvasBlob = () => new Promise(res => $('#cardCanvas').toBlob(res, 'image/png'));
ACTIONS.cardOpt = el => {
  const k = el.dataset.opt; DS.card[k] = el.type === 'checkbox' ? el.checked : el.value;
  drawCard($('#cardCanvas'));
};
ACTIONS.copyCard = async () => {
  try { const b = await canvasBlob(); await navigator.clipboard.write([new ClipboardItem({ 'image/png': b })]); toast('Image copied'); }
  catch (e) { toast('Could not copy the image. Use "Save as PNG" instead.'); }
};
ACTIONS.saveCard = async () => {
  try {
    const p = await api('save_file', 'DeepScan-specs.png', $('#cardCanvas').toDataURL('image/png'), 'image');
    if (p) toast('Saved: ' + p);
  } catch (e) { toast('Could not save the file: ' + e); }
};
ACTIONS.copyMd = () => navigator.clipboard.writeText(specMarkdown()).then(() => toast('Copied as Markdown'), () => toast('Could not copy'));

/* ---------- full report ---------- */
function buildReport() {
  const d = DS.data, hw = d.hardware, st = d.storage, net = d.network, sec = d.security, sw = d.software;
  const t = (head, rows) => `<table><thead><tr>${head.map(h => `<th>${esc(h)}</th>`).join('')}</tr></thead><tbody>${rows.map(r => `<tr>${r.map(c => `<td>${esc(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  const k = rows => `<table class="kv">${rows.filter(r => r[1] !== undefined && r[1] !== '').map(r => `<tr><th>${esc(r[0])}</th><td>${esc(r[1])}</td></tr>`).join('')}</table>`;
  const score = healthScore(), f = findings();
  let h = `<h1>DeepScan report</h1><p class="m">${esc(hw ? hw.device.name : '')} · ${new Date().toLocaleString()}</p>
    <div class="score"><b>${score}</b><span>${healthWord(score)}</span></div>
    <h2>Findings</h2>${t(['Level', 'Finding', 'Advice'], f.map(x => [FIND_BADGE[x.kind][1], x.title, x.sub || '']))}`;
  if (hw) h += `<h2>Hardware</h2>${k([['Computer', hw.device.name], ['Manufacturer', [hw.device.maker, hw.device.model].filter(Boolean).join(' ')], ['Windows', `${hw.os.name} ${hw.os.version} (build ${hw.os.build}, ${hw.os.arch})`], ['Processor', hw.cpu.name], ['Cores / threads', `${hw.cpu.cores} / ${hw.cpu.threads}`], ['Graphics', hw.gpu.map(g => g.name).join(', ')], ['Memory', `${hw.memory.total_gb} GB`], ['Motherboard', [hw.board.maker, hw.board.model].join(' ')], ['BIOS', `${hw.board.bios_version} (${hw.board.bios_date})`], ['TPM', hw.tpm], ['Secure Boot', hw.secure_boot], ['Serial number', hw.device.serial]])}
    ${t(['Memory slot', 'Size', 'Type', 'Maker'], hw.memory.modules.map(m => [m.slot, m.size_gb + ' GB', m.type + (m.speed ? ' ' + m.speed : ''), m.maker]))}`;
  if (st) h += `<h2>Storage</h2>${t(['Drive', 'Type', 'Size', 'Health', 'Temp', 'Power-on hours', 'Read errors'], st.disks.map(x => [x.name, x.type, x.size_gb + ' GB', x.status, x.temp_c != null ? x.temp_c + ' °C' : '-', x.power_on_hours ?? '-', x.read_errors ?? '-']))}
    ${t(['Volume', 'File system', 'Size', 'Free', 'Encryption'], st.volumes.map(v => [`${v.letter}: ${v.label}`, v.fs, v.size_gb + ' GB', v.free_gb + ' GB', v.encryption]))}`;
  if (net && net.active) h += `<h2>Network</h2>${k([['Connection', net.active.type], ['IPv4', net.active.ipv4], ['Subnet mask', net.active.mask], ['Gateway', net.active.gateway], ['DNS', net.active.dns.join(', ')], ['MAC', net.active.mac], ['Link speed', net.active.speed]])}
    ${t(['Target', 'Ping', 'Loss'], net.ping.map(p => [`${p.label} (${p.target})`, p.ms == null ? 'no reply' : p.ms + ' ms', p.loss + '%']))}`;
  const sp = speedAvg(); if (sp) h += `<p>Average of last ${sp.n} speed tests: download ${Math.round(sp.down)} Mbps, upload ${Math.round(sp.up)} Mbps, ping ${Math.round(sp.ping)} ms.</p>`;
  if (sec) h += `<h2>Security (score ${sec.score})</h2>${t(['Check', 'Result', 'Status', 'How to fix'], sec.checks.map(c => [c.name, c.detail, STATUS[c.status][1], c.advice]))}`;
  if (sw) h += `<h2>Startup programs</h2>${t(['Name', 'Location', 'State'], sw.startup.map(s => [s.name, s.location, s.enabled ? 'On' : 'Off']))}
    <h2>Installed programs (${sw.programs.length})</h2>${t(['Name', 'Publisher', 'Version', 'Installed'], sw.programs.map(p => [p.name, p.publisher, p.version, p.installed]))}`;
  const css = 'body{font:14px/1.5 "Segoe UI",system-ui,sans-serif;color:#14202B;max-width:980px;margin:32px auto;padding:0 24px}h1{margin:0}h2{margin:32px 0 8px;font-size:18px;border-bottom:1px solid #DDE3E9;padding-bottom:6px}.m{color:#53616F;margin:4px 0 16px}.score{display:flex;align-items:baseline;gap:12px}.score b{font-size:44px}.score span{font-size:18px;color:#53616F}table{width:100%;border-collapse:collapse;margin:8px 0;font-size:13px}th,td{text-align:left;padding:7px 10px;border-bottom:1px solid #E3E8ED;vertical-align:top}thead th{background:#F3F5F8}table.kv th{width:220px;background:none;color:#53616F;font-weight:500}@media print{body{margin:0}h2{break-after:avoid}tr{break-inside:avoid}}';
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>DeepScan report</title><style>${css}</style></head><body>${h}</body></html>`;
}
ACTIONS.saveReport = async () => {
  try {
    const p = await api('save_file', `DeepScan-report-${new Date().toISOString().slice(0, 10)}.html`, buildReport(), 'text');
    if (p) toast('Saved: ' + p);
  } catch (e) { toast('Could not save the file: ' + e); }
};
ACTIONS.printReport = () => {
  const fr = document.createElement('iframe'); fr.style.cssText = 'position:fixed;width:0;height:0;border:0;visibility:hidden'; fr.srcdoc = buildReport();
  fr.onload = () => { try { fr.contentWindow.focus(); fr.contentWindow.print(); } catch (e) { toast('Printing is not available here. Use Save as HTML.'); } setTimeout(() => fr.remove(), 60000); };
  document.body.appendChild(fr);
};

VIEWS.reports = {
  title: 'Reports',
  render() {
    const head = pageHead('Reports', 'Share your PC specs or save a full report');
    if (!DS.data.hardware) return head + `<div class="card empty"><h2>Run a scan first</h2><p class="hint">Reports are built from the latest scan.</p><button class="btn primary" data-action="scan" type="button">Run scan</button></div>`;
    const o = DS.card;
    const opt = (k, label) => `<label class="opt"><input type="checkbox" data-change="cardOpt" data-opt="${k}" ${o[k] ? 'checked' : ''}>${label}</label>`;
    const radio = (k, v, label) => `<label class="opt"><input type="radio" name="${k}" value="${v}" data-change="cardOpt" data-opt="${k}" ${o[k] === v ? 'checked' : ''}>${label}</label>`;
    return head + `<div class="row"><div class="card preview grow2"><canvas id="cardCanvas" role="img" aria-label="Preview of the spec card"></canvas><span class="card-note">Hidden from the card unless you tick it: device name, IP address, serial number</span></div>
      <div class="card grow3 stack" style="gap:18px;max-width:420px"><h2 style="margin:0">Spec card</h2>
      <fieldset><legend>Layout</legend>${radio('layout', 'landscape', 'Landscape (forums, chat)')}${radio('layout', 'square', 'Square (social media)')}</fieldset>
      <fieldset><legend>Theme</legend>${radio('theme', 'dark', 'Dark')}${radio('theme', 'light', 'Light')}</fieldset>
      <fieldset><legend>Include</legend>${opt('score', 'Health score')}${opt('board', 'Motherboard')}${opt('device', 'Device name')}${opt('ip', 'IP address')}${opt('serial', 'Serial number')}</fieldset>
      <div class="stack" style="gap:10px"><button class="btn primary" data-action="copyCard" type="button">Copy image</button><button class="btn" data-action="saveCard" type="button">Save as PNG</button><button class="btn" data-action="copyMd" type="button">Copy as Markdown</button></div></div></div>
      <div class="card"><h2>Full report</h2><p class="hint" style="margin-bottom:14px">Everything from the latest scan in one page: hardware, storage, network, security and software. It includes your device name and serial number, so check it before you send it to anyone.</p>
      <div class="actions"><button class="btn primary" data-action="saveReport" type="button">Save as HTML</button><button class="btn" data-action="printReport" type="button">Print or save as PDF</button></div></div>`;
  },
  after() { const c = $('#cardCanvas'); if (c) drawCard(c); },
};
