/* Security tab: pause Microsoft Defender real-time protection for a set time, or until turned back on. */
DS.def = { st: null, minutes: 30, busy: '', timer: null };
const DF = DS.def;
const DEF_CHOICES = [[15, '15 minutes'], [30, '30 minutes'], [60, '1 hour'], [120, '2 hours'], [0, 'Until I turn it back on']];

async function defRefresh() {
  try { DF.st = await api('defender_status'); } catch (e) { DF.st = { available: false, error: String(e) }; }
  // No periodic polling at all (every read starts PowerShell). When a timed pause is running, read once more right after it ends.
  if (DF.timer) { clearTimeout(DF.timer); DF.timer = null; }
  const s = DF.st;
  if (s && s.available && !s.realtime && !s.indefinite && s.remaining_min != null) {
    DF.timer = setTimeout(() => { DF.timer = null; defRefresh(); }, s.remaining_min * 60000 + 8000);
  }
  defPaint();
}
function defPaint() { if (DS.route === 'security' && DS.data.security) render('security', false); }
function defInit() { defRefresh(); } // once per visit to the Security tab

function defCard() {
  const s = DF.st;
  const head = (badgeHtml, sub) => `<div class="card-head"><div class="stack" style="gap:2px"><h2 style="margin:0">Microsoft Defender real-time protection</h2><span class="card-note">${sub}</span></div>${badgeHtml}</div>`;
  if (!s) return `<div class="card">${head(badge('info', 'Checking', true), 'Reading Defender status...')}</div>`;
  if (!s.available) return `<div class="card">${head(badge('info', 'Not active', true), 'Microsoft Defender is not the active antivirus on this PC (another antivirus may be in charge), so there is nothing to pause here.')}</div>`;
  if (DF.busy) return `<div class="card">${head(badge('info', 'Working', true), '')}<p class="hint" role="status"><span class="spinner"></span>&nbsp; ${esc(DF.busy)}</p></div>`;
  if (!s.realtime) {
    const when = s.indefinite ? 'It stays off until you turn it back on.' : s.until ? `It turns on again automatically at ${esc(s.until)} (in about ${s.remaining_min} min).` : 'It is off.';
    const mine = s.paused_by_app ? '' : ' It was turned off outside DeepScan.';
    return `<div class="alert"><div class="grow"><b>Real-time protection is OFF</b><span>${when}${mine} Avoid browsing and opening files until it is back on.</span></div>
      <button class="btn primary" data-action="defResume" type="button">Turn protection back on</button></div>`;
  }
  const opts = DEF_CHOICES.map(([m, l]) => `<option value="${m}" ${DF.minutes === m ? 'selected' : ''}>${esc(l)}</option>`).join('');
  const tamper = s.tamper ? `<p class="hint" style="margin-top:12px"><b>Tamper Protection is on.</b> Windows blocks programs from turning Defender off while it is on. Turn it off in Windows Security > Virus &amp; threat protection > Manage settings, then come back here.</p>` : '';
  return `<div class="card">${head(badge('ok', 'On', true), 'Use this for a short time, for example to install or test software that Defender blocks.')}
    <div class="rp-bar"><label class="grow" for="defMinutes" style="flex:0 0 auto">Turn off for</label>
      <select id="defMinutes" class="btn" data-change="defMinutes" style="min-height:44px">${opts}</select>
      <button class="btn" data-action="defPause" type="button" ${s.tamper ? 'disabled' : ''}>Turn off protection</button></div>${tamper}</div>`;
}

ACTIONS.defMinutes = el => { DF.minutes = +el.value; };
ACTIONS.defPause = async () => {
  if (DF.busy) return;
  const label = (DEF_CHOICES.find(c => c[0] === DF.minutes) || DEF_CHOICES[1])[1];
  const ok = await confirmBox({ title: 'Turn off Defender real-time protection?', ok: 'Turn off', danger: true,
    body: `<p>Real-time protection will be off ${DF.minutes ? `for <b>${esc(label)}</b> and then turn on again automatically, even if you close DeepScan` : '<b>until you turn it back on</b>'}.</p>
      <p><b>Your PC will not be protected from malware while it is off.</b> Do not browse the web or open unknown files.</p>` });
  if (!ok) return;
  DF.busy = 'Turning protection off...'; defPaint();
  let r; try { r = await api('defender_pause', DF.minutes); } catch (e) { r = { ok: false, error: String(e) }; }
  DF.busy = ''; await defRefresh();
  if (!r.ok) infoBox(r.tamper ? 'Tamper Protection is on' : 'Could not turn protection off', `<p>${esc(r.error || 'Unknown error')}</p>`);
  else toast(DF.minutes ? `Protection is off for ${label}` : 'Protection is off');
};
ACTIONS.defResume = async () => {
  if (DF.busy) return;
  DF.busy = 'Turning protection back on...'; defPaint();
  let r; try { r = await api('defender_resume'); } catch (e) { r = { ok: false, error: String(e) }; }
  DF.busy = ''; await defRefresh();
  if (!r.ok) infoBox('Could not confirm', `<p>${esc(r.error || 'Unknown error')}</p>`); else toast('Protection is back on');
};
