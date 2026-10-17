import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { type VerifiedHtmlResources } from "@waku/client";
import {
  prepareHtmlResourceDocument,
  resourceReferences,
  resourceBase64,
} from "./html-resource-document";
import { readArtifactHtmlResources } from "./html-resource-readers";
const digest = async (b: Uint8Array) =>
  createHash("sha256").update(b).digest("hex");
const hash = (b: Uint8Array) => createHash("sha256").update(b).digest("hex");
function resources(
  html: string,
  extras: [string, string, string | Uint8Array][] = [],
): VerifiedHtmlResources {
  const files = [["pages/経緯.html", "text/html", html], ...extras] as [
    string,
    string,
    string | Uint8Array,
  ][];
  const sources = files.map((f) =>
    typeof f[2] === "string" ? new Uint8Array(Buffer.from(f[2])) : f[2],
  );
  return {
    manifest: {
      version: 1,
      artifactId: "e030a29e-9fa7-4b32-ab23-9ff795a9c4cf",
      artifactSha256: "a".repeat(64),
      entryPath: files[0]![0],
      totalBytes: sources.reduce((n, b) => n + b.length, 0),
      sha256: "b".repeat(64),
      sources: files.map((f, i) => ({
        index: i,
        path: f[0],
        mediaType: f[1],
        bytes: sources[i]!.length,
        sha256: hash(sources[i]!),
      })),
    },
    sources,
  };
}
const decode = (
  doc: Awaited<ReturnType<typeof prepareHtmlResourceDocument>>,
  path: string,
) =>
  Buffer.from(
    doc.files.find((f) => f.path === path)!.data,
    "base64",
  ).toString();
test("retained CSS, external SVG, document names and relative font identities stay separate", async () => {
  const css =
    "@property --chosen {syntax:'*';inherits:false;initial-value:flight} @keyframes flight{to{opacity:.5}} .item{--chosen:var(--input);animation-name:var(--chosen);font-family:var(--font)} @font-face{font-family:Demo;src:url(font.woff2)}";
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg"><style>:root{--paint:red}.piece{fill:var(--paint)}</style><symbol id="mark"><path class="piece" d="M0 0h10"/></symbol><use href="#mark"/></svg>';
  const set = resources(
    '<link rel="stylesheet" href="../theme.css"><svg><use href="../icons.svg#mark"/></svg><math><mi>x</mi></math>',
    [
      ["theme.css", "text/css", css],
      ["icons.svg", "image/svg+xml", svg],
      ["font.woff2", "font/woff2", new Uint8Array([0, 1, 255])],
    ],
  );
  const doc = await prepareHtmlResourceDocument(set, () => true);
  expect(doc.files.map((f) => f.path)).toEqual(
    set.manifest.sources.map((s) => s.path),
  );
  expect(decode(doc, "theme.css")).toBe(css);
  expect(decode(doc, "icons.svg")).toBe(svg);
  const html = decode(doc, doc.entryPath);
  expect(html).toContain('href="../theme.css"');
  expect(html).toContain('href="../icons.svg#mark"');
  expect(html).toContain("<math><mi>x</mi></math>");
  expect(html).toContain("script-src 'none'");
  expect(doc.files[3]!.data).toBe("AAH/");
});
test("entry sanitizer preserves pinned stylesheet/picture references and removes active authority", async () => {
  const set = resources(
    '<meta http-equiv="refresh" content="0;url=https://bad"><base href="https://bad"><script>alert(1)</script><link rel="stylesheet" href="../ok.css"><link rel="stylesheet" href="../plot.png"><link rel="preload" href="../ok.css"><img src="../plot.png" srcset="../plot.png 1x, ../big.png 2x" onerror="bad()"><a href="#chart">Chart</a><a href="https://bad">External</a><form action="https://bad"><input autofocus></form>',
    [
      ["ok.css", "text/css", ""],
      ["plot.png", "image/png", new Uint8Array([1])],
      ["big.png", "image/png", new Uint8Array([2])],
    ],
  );
  const doc = await prepareHtmlResourceDocument(set, () => true),
    html = decode(doc, doc.entryPath);
  expect(html).toContain('href="../ok.css"');
  expect(html).toContain('srcset="../plot.png 1x, ../big.png 2x"');
  expect(html).toContain('href="#chart"');
  expect(html).not.toMatch(
    /<script|<base|onerror|autofocus|https:\/\/bad|rel="preload"/,
  );
  expect(html).toContain('disabled=""');
  expect(doc.files[1]!.data).toBe("");
});
test("relative resource resolution honors Unicode, spaces, source location and exact MIME", () => {
  const set = resources("x", [
    ["images/my plot🚀.svg", "image/svg+xml", "<svg/>"],
    ["theme.css", "text/css", ""],
  ]);
  const read = resourceReferences(set, "pages/経緯.html");
  expect(
    read("../images/my plot🚀.svg#point", (mime) => mime === "image/svg+xml"),
  ).toBe(true);
  for (const value of [
    "https://outside.invalid/x",
    "//outside.invalid/x",
    "../../images/my%20plot🚀.svg",
    "../missing.svg",
    "../images%2fmy%20plot🚀.svg",
    "../theme.css?q=1",
    "file:///private",
    "../theme.css\n",
  ])
    expect(read(value, () => true)).toBe(false);
  expect(read("../theme.css", (mime) => mime === "image/png")).toBe(false);
});
test("raw SVG resources refuse active, external, foreign or XML authority before hosting", async () => {
  for (const svg of [
    '<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///private">]><svg xmlns="http://www.w3.org/2000/svg"/>',
    '<?xml-stylesheet href="https://bad" type="text/css"?><svg xmlns="http://www.w3.org/2000/svg"/>',
    '<svg xmlns="http://www.w3.org/2000/svg"><script>bad()</script></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" onload="bad()"/>',
    '<svg xmlns="http://www.w3.org/2000/svg"><foreignObject/></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" xml:base="https://bad"/>',
    '<svg xmlns="http://www.w3.org/2000/svg"><use href="https://bad/x#y"/></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><image href="file:///private"/></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><animate attributeName="href" values="https://bad"/></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><g></svg>',
  ])
    await expect(
      prepareHtmlResourceDocument(
        resources("<svg/>", [["icons.svg", "image/svg+xml", svg]]),
        () => true,
      ),
    ).rejects.toThrow();
});
test("resource preparation yields, cancels and rejects changed byte counts", async () => {
  let checks = 0;
  await expect(
    prepareHtmlResourceDocument(
      resources("<div>" + "x".repeat(300000) + "</div>"),
      () => ++checks < 8,
    ),
  ).rejects.toThrow("scope changed");
  const set = resources("x");
  set.sources[0] = new Uint8Array(0);
  await expect(prepareHtmlResourceDocument(set, () => true)).rejects.toThrow(
    "bytes changed",
  );
});
test("portable resource encoding preserves batch boundaries and empty bytes", async () => {
  const bytes = new Uint8Array(40001);
  for (let i = 0; i < bytes.length; i++) bytes[i] = i % 256;
  expect(await resourceBase64(bytes, () => true)).toBe(
    Buffer.from(bytes).toString("base64"),
  );
  expect(await resourceBase64(new Uint8Array(), () => true)).toBe("");
});
test.skipIf(!process.env.ION_HTML_RESOURCES_FIXTURE)(
  "actual native publication passes through full source verification into host files",
  async () => {
    const fixture = JSON.parse(
      readFileSync(process.env.ION_HTML_RESOURCES_FIXTURE!, "utf8"),
    );
    const original = Buffer.from(fixture.original, "base64");
    const client = {
      request: async (q: any) =>
        q.type === "readArtifactHtmlManifest"
          ? {
              type: "artifactHtmlManifest",
              result: {
                document: fixture.expected,
                manifest: fixture.manifest,
              },
            }
          : {
              type: "artifactHtmlSource",
              result: {
                document: fixture.expected,
                chunk: fixture.chunks.find(
                  (c: any) =>
                    c.sourceIndex === q.query.source.sourceIndex &&
                    c.offset === q.query.source.offset,
                ),
              },
            },
    };
    const set = await readArtifactHtmlResources(
      client as any,
      fixture.expected,
      original,
      () => true,
      digest,
    );
    expect(set).not.toBeNull();
    const doc = await prepareHtmlResourceDocument(set!, () => true);
    expect(doc.entryPath).toBe(fixture.manifest.entryPath);
    for (let i = 1; i < doc.files.length; i++)
      expect(Buffer.from(doc.files[i]!.data, "base64")).toEqual(
        Buffer.from(set!.sources[i]!),
      );
    expect(decode(doc, doc.entryPath)).toContain("Content-Security-Policy");
  },
);
