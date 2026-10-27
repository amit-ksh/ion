/** Local pairing QR helper. Credentials are processed only in the browser. */
import { fileURLToPath } from 'node:url';
const build = await Bun.build({
  entrypoints: [
    fileURLToPath(new URL('./mobile-pairing-client.ts', import.meta.url)),
  ],
  target: 'browser',
  minify: true,
});
if (!build.success) throw new Error('Could not build the pairing QR helper.');
const client = await build.outputs[0]!.text();
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Pair Ion mobile</title>
<style>body{background:#f6f7f2;color:#1b2510;font:16px/1.5 system-ui;margin:40px auto;padding:0 24px;max-width:560px}h1{font-size:32px;letter-spacing:-1px}label{display:block;margin:16px 0}input{display:block;box-sizing:border-box;width:100%;font:inherit;padding:12px;border:1px solid #b7bfb0;border-radius:10px;background:#fff}button{font:600 16px system-ui;border:0;border-radius:10px;padding:14px 20px;background:#c2ef68;color:#1b2510;cursor:pointer}button+button{background:#e3e8dc;margin-left:8px}canvas{max-width:100%;height:auto}#result{background:white;padding:16px;margin-top:24px;border-radius:16px;text-align:center}#error{color:#972b20}small{color:#536147}</style>
<h1>Pair your phone with Ion</h1><p>Enable the remote daemon in Ion Desktop → Settings → Daemon. Copy its address and token here, then open <strong>Pair desktop → Scan to pair</strong> on your phone.</p>
<form autocomplete="off"><label>Desktop name (optional)<input name="name" maxlength="80" placeholder="My desktop"></label><label>WebSocket address<input name="address" required placeholder="ws://192.168.1.5:4096" spellcheck="false" autocapitalize="off"></label><label>Daemon token<input name="token" type="password" required maxlength="4096"></label><button type="submit">Show pairing QR</button><button type="button" id="clear">Clear</button></form>
<p id="error" role="alert"></p><div id="result" hidden><canvas aria-label="Ion desktop pairing QR code"></canvas><p>Scan this code, review the details, then tap Add.</p></div>
<p><small>This helper runs on this computer. Your token stays in this browser’s memory and is never sent to the helper server. The QR grants full control of this daemon; show it only to your own phone. Keep both devices on the same private network, or use your private tailnet address.</small></p><script type="module" src="/client.js"></script></html>`;
const server = Bun.serve({
  hostname: '127.0.0.1',
  port: 4197,
  fetch(request) {
    const url = new URL(request.url);
    if (
      request.method !== 'GET' ||
      request.headers.get('host') !== '127.0.0.1:4197' ||
      url.search
    )
      return new Response('Not found', { status: 404 });
    const headers = {
      'Cache-Control': 'no-store',
      'Referrer-Policy': 'no-referrer',
      'Content-Security-Policy':
        "default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; connect-src 'none'; img-src 'self' data:; form-action 'none'; frame-ancestors 'none'",
    };
    if (url.pathname === '/')
      return new Response(html, {
        headers: { ...headers, 'Content-Type': 'text/html; charset=utf-8' },
      });
    if (url.pathname === '/client.js')
      return new Response(client, {
        headers: { ...headers, 'Content-Type': 'text/javascript' },
      });
    return new Response('Not found', { status: 404 });
  },
});
console.log(
  `Open ${server.url} on your desktop to create a private pairing QR. Press Ctrl+C when finished.`,
);
