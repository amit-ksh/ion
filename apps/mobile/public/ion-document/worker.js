/* Dedicated document scope. Never proxies a request or persists document bytes. */
const sessions = new Map();
const prefix = "/ion-document/";
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const mimeTypes = new Set([
  "text/css",
  "image/svg+xml",
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "font/woff",
  "font/woff2",
  "font/ttf",
  "font/otf",
]);
function validPath(value) {
  return (
    typeof value === "string" &&
    value &&
    new TextEncoder().encode(value).length <= 512 &&
    !/[\\:?#\x00-\x1f\x7f-\x9f]/.test(value) &&
    !/[\uD800-\uDFFF]/u.test(value) &&
    !value.split("/").some((part) => !part || part === "." || part === "..")
  );
}
function prune() {
  for (const [token, session] of sessions)
    if (session.expires <= performance.now()) sessions.delete(token);
}
function policy(token) {
  const path = self.location.origin + prefix + token + "/";
  return (
    "default-src 'none'; script-src 'none'; style-src 'unsafe-inline' data: " +
    path +
    "; img-src data: " +
    path +
    "; font-src data: " +
    path +
    "; connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'; sandbox allow-same-origin"
  );
}
self.addEventListener("install", (event) =>
  event.waitUntil(self.skipWaiting()),
);
self.addEventListener("activate", (event) =>
  event.waitUntil(self.clients.claim()),
);
self.addEventListener("message", (event) => {
  prune();
  const port = event.ports?.[0];
  try {
    const input = event.data,
      source = event.source;
    if (
      !port ||
      !source?.id ||
      new URL(source.url).origin !== self.location.origin ||
      new URL(source.url).pathname.startsWith(prefix) ||
      !input ||
      !uuid.test(input.token)
    )
      throw Error("Invalid document host owner");
    const old = sessions.get(input.token);
    if (input.type === "close") {
      if (old && old.owner !== source.id)
        throw Error("Document host owner changed");
      sessions.delete(input.token);
      port.postMessage({ ok: true, value: null });
      return;
    }
    if (input.type === "renew") {
      if (old && old.owner !== source.id)
        throw Error("Document host owner changed");
      if (old) old.expires = performance.now() + 30_000;
      port.postMessage({ ok: true, value: !!old });
      return;
    }
    if (
      input.type !== "open" ||
      old ||
      sessions.size >= 2 ||
      !validPath(input.entry) ||
      !Array.isArray(input.files) ||
      !input.files.length ||
      input.files.length > 33
    )
      throw Error("Document host unavailable");
    const files = new Map();
    let encoded = 0,
      extra = 0;
    for (const file of input.files) {
      if (
        !file ||
        Object.keys(file).sort().join(",") !== "data,mediaType,path" ||
        !validPath(file.path) ||
        files.has(file.path) ||
        typeof file.data !== "string"
      )
        throw Error("Invalid document file");
      encoded += file.data.length;
      if (
        encoded > 56 * 1024 * 1024 ||
        file.data.length % 4 ||
        !/^[A-Za-z0-9+/]*={0,2}$/.test(file.data)
      )
        throw Error("Invalid document bytes");
      const raw = atob(file.data),
        html = file.path === input.entry;
      if (
        html
          ? file.mediaType !== "text/html" ||
            !raw.length ||
            raw.length > 32 * 1024 * 1024
          : !mimeTypes.has(file.mediaType) ||
            raw.length > 2 * 1024 * 1024 ||
            (!raw.length && file.mediaType !== "text/css")
      )
        throw Error("Invalid document resource");
      if (!html) extra += raw.length;
      if (extra > 8 * 1024 * 1024)
        throw Error("Document resource bound exceeded");
      const bytes = Uint8Array.from(raw, (char) => char.charCodeAt(0));
      files.set(file.path, { bytes, mediaType: file.mediaType });
    }
    if (!files.has(input.entry)) throw Error("Missing document entry");
    sessions.set(input.token, {
      files,
      owner: source.id,
      expires: performance.now() + 30_000,
    });
    const path = input.entry.split("/").map(encodeURIComponent).join("/");
    port.postMessage({
      ok: true,
      value: {
        token: input.token,
        url: self.location.origin + prefix + input.token + "/" + path,
      },
    });
  } catch {
    port?.postMessage({ ok: false });
  }
});
function response(request) {
  prune();
  const url = new URL(request.url);
  let file, token;
  if (
    url.origin === self.location.origin &&
    !url.search &&
    ["GET", "HEAD"].includes(request.method) &&
    url.pathname.startsWith(prefix)
  ) {
    const route = url.pathname.slice(prefix.length),
      slash = route.indexOf("/");
    token = route.slice(0, slash);
    if (slash > 0 && uuid.test(token)) {
      try {
        const encoded = route.slice(slash + 1);
        if (!/%(?:2f|5c|00)/i.test(encoded)) {
          const path = decodeURIComponent(encoded);
          if (validPath(path)) file = sessions.get(token)?.files.get(path);
        }
      } catch {}
    }
  }
  const headers = {
    "Content-Type": file
      ? file.mediaType +
        (["text/html", "text/css", "image/svg+xml"].includes(file.mediaType)
          ? "; charset=utf-8"
          : "")
      : "text/plain",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
    "Referrer-Policy": "no-referrer",
    "Content-Security-Policy":
      token && file ? policy(token) : "default-src 'none'; sandbox",
  };
  return new Response(file && request.method !== "HEAD" ? file.bytes : null, {
    status: file ? 200 : 404,
    headers,
  });
}
self.addEventListener("fetch", (event) =>
  event.respondWith(response(event.request)),
);
