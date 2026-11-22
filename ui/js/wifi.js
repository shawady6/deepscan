/* Saved Wi-Fi passwords (Network tab). Read only when the button is pressed. Passwords stay in memory on this screen:
   they are hidden by default, never saved by DeepScan, and cleared with the Clear button or when the app closes. */
DS.wifi = { list: null, busy: false, shown: new Set() };
const wifiPaint = () => { if (DS.route === 'network') render('network', false); };

function wifiCard() {
  const W = DS.wifi;
  const btn = `<button class="btn small ${W.list ? '' : 'primary'}" data-action="wifiLoad" type="button" ${W.busy ? 'disabled' : ''}>${W.list ? 'Read again' : 'Show saved networks'}</button>${W.list ? '<button class="btn small" data-action="wifiClear" type="button">Clear</button>' : ''}`;
  let body;
  if (W.busy) body = `<p class="hint"><span class="spinner"></span>&nbsp; Reading saved Wi-Fi networks...</p>`;
  else if (!W.list) body = `<p class="hint">Lists the Wi-Fi networks this PC has connected to before and their saved passwords, the same as running <span class="mono">netsh wlan show profile name=... key=clear</span>. Shown only on this screen, only when you ask.</p>`;
  else if (W.list.error) body = `<p class="hint">${esc(W.list.error)}</p>`;
  else if (!W.list.networks.length) body = `<p class="hint">No saved Wi-Fi networks on this PC.</p>`;
  else body = `<div class="tw"><table class="t" style="min-width:520px"><thead><tr><th>Network</th><th>Security</th><th>Password</th><th></th></tr></thead><tbody>${W.list.networks.map((n, i) => {
    const shown = W.shown.has(i);
    const pw = n.password ? (shown ? `<span class="mono">${esc(n.password)}</span>` : '<span class="mono muted">••••••••</span>') : '<span class="muted">None saved (open or enterprise network)</span>';
    const acts = n.password ? `<button class="btn small" data-action="wifiToggle" data-id="${i}" type="button">${shown ? 'Hide' : 'Show'}</button> <button class="btn small" data-action="wifiCopy" data-id="${i}" type="button">Copy</button>` : '';
    return `<tr><td>${esc(n.name)}${n.hidden ? ' <span class="muted">(hidden)</span>' : ''}</td><td>${esc(n.auth || '-')}</td><td>${pw}</td><td class="num" style="white-space:nowrap">${acts}</td></tr>`;
  }).join('')}</tbody></table></div>${W.list.demo ? '<p class="hint">Sample data.</p>' : ''}`;
  return `<div class="card"><div class="card-head"><h2>Saved Wi-Fi passwords</h2><div style="display:flex;gap:8px">${btn}</div></div>${body}</div>`;
}
ACTIONS.wifiLoad = async () => {
  DS.wifi.busy = true; DS.wifi.shown = new Set(); wifiPaint();
  try { DS.wifi.list = await api('wifi_profiles'); } catch (e) { DS.wifi.list = { error: String(e), networks: [] }; }
  DS.wifi.busy = false; wifiPaint();
};
ACTIONS.wifiClear = () => { DS.wifi.list = null; DS.wifi.shown = new Set(); wifiPaint(); };
ACTIONS.wifiToggle = el => { const i = +el.dataset.id; DS.wifi.shown.has(i) ? DS.wifi.shown.delete(i) : DS.wifi.shown.add(i); wifiPaint(); };
ACTIONS.wifiCopy = el => { const n = DS.wifi.list && DS.wifi.list.networks[+el.dataset.id]; if (n && n.password) copyText(n.password); };
