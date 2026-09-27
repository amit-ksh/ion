/** Static owner console. Secrets remain in the page's memory, never in URLs or storage. */
export const ownerHtml = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>ion personal gateway</title><link rel="stylesheet" href="/owner/style.css"></head>
<body><main>
  <header><h1>Personal gateway</h1><p>Review access and external actions on your own gateway.</p></header>
  <section id="sign-in"><h2>Owner access</h2><label for="owner-token">Owner token</label><div class="inline"><input id="owner-token" type="password" autocomplete="off" spellcheck="false"><button id="connect">Unlock</button></div><p>Your token stays in this tab's memory. Closing the tab clears it.</p></section>
  <div id="console" hidden>
    <nav><button id="refresh">Refresh</button><button id="lock">Lock</button></nav><p id="notice" role="status" aria-live="polite"></p>
    <section><h2>Pair a device</h2><p>Create a one-use invite valid for five minutes. Share it with the device only over a trusted channel. Grant only the actions that device needs.</p><div class="inline"><label><input id="inspect" type="checkbox" checked> Inspect own actions</label><label><input id="propose" type="checkbox"> Propose writes</label><label><input id="review" type="checkbox"> Review approvals</label><label><input id="task-read" type="checkbox"> Read tasks</label><label><input id="task-control" type="checkbox"> Control tasks</label><label><input id="publish" type="checkbox"> Publish desktop state</label><button id="invite">Create invite</button></div><div id="invite-result" hidden><label for="invite-secret">Copy this invite now</label><input id="invite-secret" readonly><p>It will disappear when you lock or refresh this tab.</p></div></section>
    <section><h2>Pending approvals</h2><p>Check the connected account, target, arguments and scopes before allowing an action.</p><div id="proposals"></div></section>
    <section><h2>Action history</h2><div id="history"></div></section>
    <section><h2>Paired devices</h2><div id="devices"></div></section>
    <section><h2>Connected accounts</h2><div id="accounts"></div></section>
  </div>
</main><script src="/owner/app.js" defer></script></body></html>`;

export const ownerCss = `:root{color-scheme:light dark;font-family:system-ui,-apple-system,sans-serif;background:#f6f5f2;color:#222722}*{box-sizing:border-box}body{margin:0}main{max-width:880px;margin:auto;padding:36px 20px 80px}header{padding-bottom:24px;border-bottom:1px solid #b8c3b7}h1{font-size:2rem;letter-spacing:-.025em;margin:0 0 8px}h2{font-size:1.2rem;margin:0 0 12px}p{line-height:1.5;color:#46544a}section{padding:24px 0;border-bottom:1px solid #b8c3b7}label{font-weight:600}input:not([type=checkbox]){font:inherit;padding:10px 12px;border:1px solid #728476;border-radius:7px;min-width:0;flex:1;background:#fff;color:#19221b}input:focus-visible,button:focus-visible{outline:3px solid #2b7554;outline-offset:2px}button{font:inherit;font-weight:600;border:1px solid #536d5b;border-radius:7px;background:#eaf0e8;color:#183c2b;padding:9px 14px;cursor:pointer;min-height:42px}button:hover{background:#d9e7db}button:disabled{opacity:.5;cursor:wait}.inline,nav{display:flex;gap:12px;align-items:center;flex-wrap:wrap}.inline>label{display:flex;gap:7px;align-items:center}.inline input[type=checkbox]{width:18px;height:18px}.item{padding:16px 0;border-top:1px solid #cad4c9}.item:first-child{border-top:0}.item h3{font-size:1rem;margin:0 0 5px}.meta{font-size:.9rem;margin:5px 0}.actions{display:flex;gap:10px;flex-wrap:wrap;margin-top:12px}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#e9eee7;color:#172b1d;padding:12px;border-radius:7px;max-height:280px;overflow:auto}code{overflow-wrap:anywhere}#notice{min-height:24px;font-weight:600}#invite-result{margin-top:16px}#invite-result label{display:block;margin-bottom:8px}#invite-result input{width:100%}[hidden]{display:none!important}@media(max-width:580px){main{padding:24px 16px 60px}.inline>input:not([type=checkbox]){flex-basis:100%}.inline>button{width:100%}}@media(prefers-color-scheme:dark){:root{background:#151a16;color:#e8eee7}p{color:#bccbbc}header,section{border-color:#3c4e40}input:not([type=checkbox]){background:#222b24;color:#e8eee7;border-color:#78947d}button{background:#284230;color:#e5f4e8;border-color:#75967b}button:hover{background:#35583f}.item{border-color:#3c4e40}pre{background:#202c22;color:#e6f0e6}}`;

export const ownerJs = String.raw`'use strict';
const $ = id => document.getElementById(id);
let token = '';
function notice(message) { $('notice').textContent = message; }
function node(tag, text, className) { const el = document.createElement(tag); if (text != null) el.textContent = String(text); if (className) el.className = className; return el; }
function button(label, action) { const el = node('button', label); el.type = 'button'; el.addEventListener('click', async () => { el.disabled = true; try { await action(); } catch (e) { notice(e.message); } finally { el.disabled = false; } }); return el; }
async function api(path, method = 'GET', payload) {
  const response = await fetch(path, { method, headers: { Authorization: 'Bearer ' + token, ...(payload === undefined ? {} : { 'Content-Type': 'application/json' }) }, body: payload === undefined ? undefined : JSON.stringify(payload), cache: 'no-store', credentials: 'omit', redirect: 'error' });
  const data = await response.json();
  if (!response.ok && !(response.status === 409 && data.status === 'uncertain')) throw new Error(data.error || 'Gateway request failed (' + response.status + ')');
  return data;
}
function row(container, title, lines) { const item = node('article', null, 'item'); item.append(node('h3', title)); for (const line of lines) item.append(node('p', line, 'meta')); container.append(item); return item; }
async function refresh() {
  const [proposals, devices, accounts, allowlist] = await Promise.allSettled([api('/v1/owner/proposals'), api('/v1/owner/devices'), api('/v1/owner/connected-accounts'), api('/v1/owner/allowlist')]);
  if (proposals.status === 'rejected' || devices.status === 'rejected' || allowlist.status === 'rejected') throw new Error('Could not load owner state. Check the owner token and gateway connection.');
  $('proposals').replaceChildren(); $('history').replaceChildren(); $('devices').replaceChildren(); $('accounts').replaceChildren();
  const pending = proposals.value.proposals.filter(p => p.status === 'pending');
  if (!pending.length) $('proposals').append(node('p', 'No pending approvals.'));
  for (const p of pending) {
    const e = p.effect;
    const item = row($('proposals'), e.tool, ['Account: ' + e.accountId, 'Toolkit: ' + e.toolkit + ' · Version: ' + e.version, 'Target: ' + e.target, 'Scopes: ' + e.scopes.join(', '), 'Proposed by device: ' + p.deviceId]);
    item.append(node('pre', e.body));
    const actions = node('div', null, 'actions');
    actions.append(button('Deny', async () => { await api('/v1/owner/proposals/' + p.id + '/decision', 'POST', { fingerprint: p.fingerprint, approve: false }); await refresh(); notice('Proposal denied.'); }));
    actions.append(button('Approve exact action', async () => { if (!confirm('Approve this exact tool, account, target, arguments and scopes?')) return; await api('/v1/owner/proposals/' + p.id + '/decision', 'POST', { fingerprint: p.fingerprint, approve: true }); await refresh(); notice('Approved. Execution still requires a separate action.'); }));
    item.append(actions);
  }
  const approved = proposals.value.proposals.filter(p => p.status === 'approved');
  for (const p of approved) {
    const e = p.effect;
    const item = row($('proposals'), 'Approved · ' + e.tool, ['Account: ' + e.accountId, 'Toolkit: ' + e.toolkit + ' · Version: ' + e.version, 'Target: ' + e.target, 'Scopes: ' + e.scopes.join(', '), 'Proposed by device: ' + p.deviceId, 'Reviewed by: ' + (p.reviewedBy || 'Unknown')]);
    item.append(node('pre', e.body));
    item.append(button('Execute once', async () => { if (!confirm('Send this approved action to Composio once? An interrupted response cannot be retried automatically.')) return; const result = await api('/v1/owner/proposals/' + p.id + '/execute', 'POST'); await refresh(); notice(result.status === 'uncertain' ? 'Result uncertain. Check the connected account before any new action.' : 'Executed. Log ID: ' + result.resultReference); }));
  }
  const uncertain = proposals.value.proposals.filter(p => p.status === 'uncertain');
  for (const p of uncertain) row($('proposals'), 'Needs manual reconciliation · ' + p.effect.tool, ['Target: ' + p.effect.target, 'Do not retry automatically. Check the connected account.']);
  const history = proposals.value.proposals.filter(p => p.status === 'denied' || p.status === 'succeeded');
  if (!history.length) $('history').append(node('p', 'No completed decisions yet.'));
  for (const p of history) {
    const e = p.effect;
    row($('history'), p.status + ' · ' + e.tool, ['Account: ' + e.accountId, 'Target: ' + e.target, 'Version: ' + e.version, 'Reviewed by: ' + (p.reviewedBy || 'Unknown'), 'Log ID: ' + (p.resultReference || 'None')]);
  }
  if (!devices.value.devices.length) $('devices').append(node('p', 'No paired devices.'));
  for (const d of devices.value.devices) {
    const item = row($('devices'), d.name, ['ID: ' + d.id, 'Grants: ' + d.grants.join(', '), 'Expires: ' + new Date(d.expiresAt).toLocaleString(), d.revoked ? 'Revoked' : (Date.now() >= d.expiresAt ? 'Expired' : 'Active')]);
    if (!d.revoked) item.append(button('Revoke', async () => { if (!confirm('Revoke this device?')) return; await api('/v1/owner/devices/' + d.id + '/revoke', 'POST'); notice('Device revoked.'); await refresh(); }));
  }
  if (accounts.status === 'rejected') $('accounts').append(node('p', 'Composio account lookup unavailable. Configure a scoped project key on the gateway.'));
  else if (!accounts.value.items.length) $('accounts').append(node('p', 'No connected accounts.'));
  else for (const a of accounts.value.items) {
    const tools = allowlist.value.tools.filter(t => t.accountId === a.id);
    row($('accounts'), a.toolkit, ['Account ID: ' + a.id, 'Status: ' + a.status, 'Allowed tools and requested scopes: ' + (tools.length ? tools.map(t => t.tool + '@' + t.version + ' (' + t.scopes.join(', ') + ')').join('; ') : 'None')]);
  }
  notice('Gateway state refreshed.');
}
$('connect').addEventListener('click', async () => { token = $('owner-token').value.trim(); $('owner-token').value = ''; try { await refresh(); $('sign-in').hidden = true; $('console').hidden = false; } catch (e) { token = ''; $('owner-token').focus(); alert(e.message); } });
$('owner-token').addEventListener('keydown', e => { if (e.key === 'Enter') $('connect').click(); });
$('lock').addEventListener('click', () => { token = ''; $('invite-secret').value = ''; $('invite-result').hidden = true; $('console').hidden = true; $('sign-in').hidden = false; $('owner-token').focus(); });
$('refresh').addEventListener('click', () => { $('invite-secret').value = ''; $('invite-result').hidden = true; refresh().catch(e => notice(e.message)); });
$('invite').addEventListener('click', async () => { const grants = []; if ($('inspect').checked) grants.push('inspect'); if ($('propose').checked) grants.push('propose'); if ($('review').checked) grants.push('review'); if ($('task-read').checked) grants.push('taskRead'); if ($('task-control').checked) grants.push('taskControl'); if ($('publish').checked) grants.push('publish'); try { const result = await api('/v1/owner/invites', 'POST', { grants }); $('invite-secret').value = result.invite; $('invite-result').hidden = false; notice('Invite created. It expires in five minutes.'); } catch (e) { notice(e.message); } });`;
