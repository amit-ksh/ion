import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
const source = readFileSync(
  new URL("../../public/ion-document/worker.js", import.meta.url),
  "utf8",
);
const token = "fa03cd03-a369-4d42-a193-a2386cfe411d";
function worker() {
  const handlers = new Map<string, (e: any) => void>();
  let now = Date.now();
  class Clock extends Date {
    static now() {
      return now;
    }
  }
  runInNewContext(source, {
    self: {
      location: { origin: "https://ion.test" },
      addEventListener: (kind: string, fn: any) => handlers.set(kind, fn),
      skipWaiting: async () => {},
      clients: { claim: async () => {} },
    },
    URL,
    Response,
    Uint8Array,
    TextEncoder,
    Map,
    Set,
    Date: Clock,
    performance: { now: () => now },
    atob,
  });
  const message = (
    data: any,
    owner = "owner",
    url = "https://ion.test/artifacts",
  ) => {
    let result: any;
    handlers.get("message")!({
      data,
      source: { id: owner, url },
      ports: [{ postMessage: (v: any) => (result = v) }],
    });
    return result;
  };
  const fetch = (path: string, method = "GET") => {
    let response: Response | undefined;
    handlers.get("fetch")!({
      request: { url: "https://ion.test" + path, method },
      respondWith: (r: Response) => (response = r),
    });
    return response!;
  };
  return { message, fetch, advance: (n: number) => (now += n) };
}
function input() {
  return {
    type: "open",
    token,
    entry: "docs/経緯.html",
    files: [
      {
        path: "docs/経緯.html",
        mediaType: "text/html",
        data: btoa('<link rel=stylesheet href="../style.css">'),
      },
      {
        path: "style.css",
        mediaType: "text/css",
        data: btoa(
          "@property --n{syntax:'*';inherits:true;initial-value:demo}",
        ),
      },
      { path: "empty.css", mediaType: "text/css", data: "" },
      {
        path: "icons.svg",
        mediaType: "image/svg+xml",
        data: btoa(
          '<svg xmlns="http://www.w3.org/2000/svg"><symbol id="x"/></svg>',
        ),
      },
      { path: "font.woff2", mediaType: "font/woff2", data: "AAH/" },
    ],
  };
}
test("actual worker serves separate same-origin files with a capability-path CSP and no network fallback", async () => {
  const w = worker(),
    opened = w.message(input());
  expect(opened.ok).toBe(true);
  const path = new URL(opened.value.url).pathname;
  const html = w.fetch(path);
  expect(html.status).toBe(200);
  expect(await html.text()).toContain("../style.css");
  const root = "/ion-document/" + token + "/",
    css = w.fetch(root + "style.css");
  expect(await css.text()).toContain("@property --n");
  const csp = css.headers.get("Content-Security-Policy")!;
  expect(csp).toContain("https://ion.test" + root);
  expect(csp).toContain("script-src 'none'");
  expect(csp).toContain("sandbox allow-same-origin");
  expect(csp).not.toContain("allow-scripts");
  expect(css.headers.get("Cache-Control")).toBe("no-store");
  expect(css.headers.get("Access-Control-Allow-Origin")).toBeNull();
  expect(await w.fetch(root + "empty.css").text()).toBe("");
  expect(
    new Uint8Array(await w.fetch(root + "font.woff2").arrayBuffer()),
  ).toEqual(new Uint8Array([0, 1, 255]));
  for (const value of [
    root + "../style.css",
    root + "%2e%2e/style.css",
    root + "style.css?q=1",
    root + "docs%2f経緯.html",
    "/account/data",
    "/ion-document/wrong/style.css",
  ])
    expect(w.fetch(value).status).toBe(404);
  expect(w.fetch(root + "style.css", "POST").status).toBe(404);
  expect(await w.fetch(root + "style.css", "HEAD").text()).toBe("");
});
test("worker ownership, expiry and close refuse replay; restart retains no document bytes", () => {
  const w = worker();
  expect(w.message(input()).ok).toBe(true);
  expect(w.message({ type: "renew", token }, "other").ok).toBe(false);
  expect(w.message({ type: "close", token }, "other").ok).toBe(false);
  expect(w.message({ type: "renew", token }).value).toBe(true);
  w.advance(30001);
  expect(w.fetch("/ion-document/" + token + "/style.css").status).toBe(404);
  expect(w.message({ type: "renew", token }).value).toBe(false);
  expect(w.message(input()).ok).toBe(true);
  expect(w.message({ type: "close", token }).ok).toBe(true);
  expect(w.fetch("/ion-document/" + token + "/style.css").status).toBe(404);
  expect(worker().fetch("/ion-document/" + token + "/style.css").status).toBe(
    404,
  );
});
test("worker cannot be opened by a document, foreign origin or invalid file set", () => {
  for (const [owner, url] of [
    ["owner", "https://outside.invalid/"],
    ["owner", "https://ion.test/ion-document/" + token + "/page.html"],
    ["", "https://ion.test/"],
  ])
    expect(worker().message(input(), owner, url).ok).toBe(false);
  for (const change of [
    (v: any) => v.files.push({ ...v.files[0] }),
    (v: any) => (v.entry = "../escape.html"),
    (v: any) => (v.files[1].path = "/account"),
    (v: any) => (v.files[1].mediaType = "application/javascript"),
    (v: any) => (v.files[1].data = "bad?"),
    (v: any) => (v.files[1].data = btoa("x".repeat(2 * 1024 * 1024 + 1))),
    (v: any) => (v.files[0].mediaType = "text/css"),
    (v: any) => (v.files[1].secret = "credential"),
  ]) {
    const value = input();
    change(value);
    expect(worker().message(value).ok).toBe(false);
  }
});
