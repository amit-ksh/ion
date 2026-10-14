import { expect, test } from "bun:test";
import { parse } from "parse5";
import {
  prepareRenderedHtml,
  prepareHtmlViews,
  allowDocumentNavigation,
} from "./html-rendering";

test("preserve responsive CSS, tables, SVG and embedded raster images", async () => {
  const source = `<style>@media (max-width:500px){.grid{display:grid;grid-template-columns:1fr}} .grid{color:#123;background:linear-gradient(red,blue)}</style><div class="grid" style="padding:12px"><table><tr><td>Report</td></tr></table><svg viewBox="0 0 10 10"><path d="M0 0L10 10"/></svg><img alt="plot" src="data:image/png;base64,AAAA"></div>`;
  const { html } = await prepareRenderedHtml(source, () => true);
  expect(html).toContain("@media (max-width:500px)");
  expect(html).toContain("linear-gradient(red,blue)");
  expect(html).toContain("<tbody><tr><td>Report</td>");
  expect(html).toContain('viewBox="0 0 10 10"');
  expect(html).toContain("data:image/png;base64,AAAA");
  expect(html.indexOf("Content-Security-Policy")).toBeLessThan(
    html.indexOf("<style>"),
  );
});
test("remove active/remote content even through malformed/foreign HTML", async () => {
  const { html, removed } = await prepareRenderedHtml(
    `<meta http-equiv="refresh" content="0;url=https://evil.test"><base href="https://evil.test"><link rel="stylesheet" href="file:///private"><script>alert(1)</script><iframe srcdoc="<script>x</script>"></iframe><form action="ion://run"><button onclick="x()">Run</button><input autofocus></form><svg><foreignObject><iframe src="https://evil.test"></iframe></foreignObject><animate attributeName="href" values="javascript:x()"/><a xlink:href="java&#115;cript:x()">x</a></svg><a href="ion://run" ping="https://evil.test">link</a><img src="file:///private" onerror="x()"><style>@import "https://evil.test";.x{background:url(https://evil.test)}</style>`,
    () => true,
  );
  expect(removed).toBe(true);
  const tree = parse(html),
    pending: any[] = [tree];
  while (pending.length) {
    const node = pending.pop();
    expect(
      ["script", "iframe", "foreignObject", "animate", "base", "link"].includes(
        node.tagName,
      ),
    ).toBe(false);
    for (const attr of node.attrs ?? []) {
      expect(attr.name.startsWith("on")).toBe(false);
      expect(
        ["href", "src", "srcdoc", "ping", "action", "autofocus"].includes(
          attr.name,
        ),
      ).toBe(false);
    }
    pending.push(...(node.childNodes ?? []));
  }
  expect(html).toContain("default-src 'none'");
  expect(html).toContain("script-src 'none'");
  expect(html).toContain('<button disabled="">');
});
test("HTML5 reserialization does not restore mutation-XSS markup", async () => {
  for (const source of [
    `<math><mtext><table><mglyph><style><!--</style><img title="--><img src=x onerror=alert(1)>">`,
    `<svg><style><img src=x onerror=alert(1)></style></svg>`,
    `<noscript><p title="</noscript><img src=x onerror=alert(1)>">`,
  ]) {
    const { html } = await prepareRenderedHtml(source, () => true);
    const parsed: any = parse(html),
      pending: any[] = [parsed];
    while (pending.length) {
      const node = pending.pop();
      for (const attr of node.attrs ?? [])
        expect(attr.name.startsWith("on")).toBe(false);
      pending.push(...(node.childNodes ?? []));
    }
  }
});
test("larger styled publications retain complete source", async () => {
  const text =
    "<style>.page{display:flex}</style><p>" + "hello ".repeat(20000) + "</p>";
  const result = await prepareHtmlViews(text, () => true);
  expect(result.rendered?.html).toContain("hello ".repeat(20000));
  expect(result.sourceRows.map((row) => row.text).join("")).toBe(text);
});
test("bounds use UTF8 bytes and cancellation stops preparation", async () => {
  await expect(
    prepareRenderedHtml("語".repeat(350000), () => true),
  ).rejects.toThrow("1 MiB");
  let checks = 0;
  await expect(
    prepareRenderedHtml("x".repeat(60000), () => ++checks < 3),
  ).rejects.toThrow("scope changed");
  await expect(
    prepareRenderedHtml(
      "<div>".repeat(150) + "text" + "</div>".repeat(150),
      () => true,
    ),
  ).rejects.toThrow("too complex");
});
test("native navigation accepts only the initial blank document and anchors", () => {
  for (const url of ["about:blank", "about:blank#section"])
    expect(allowDocumentNavigation(url)).toBe(true);
  for (const url of [
    "https://evil.test",
    "file:///private",
    "javascript:alert(1)",
    "ion://run",
    "data:text/html,x",
    "about:blank?redirect=1",
  ])
    expect(allowDocumentNavigation(url)).toBe(false);
});

test("invalid attribute names cannot create active attributes during reserialization", async () => {
  const { html } = await prepareRenderedHtml(
    '<div bad"name="value" onclick="x()">visible</div>',
    () => true,
  );
  expect(html).not.toContain('bad"name');
  expect(html).not.toContain("onclick");
});
test("source batches retain supplementary Unicode without splitting codepoints", async () => {
  const source = "a".repeat(999) + "🚀" + "b".repeat(999) + "🚀";
  const result = await prepareHtmlViews(source, () => true);
  expect(result.sourceRows.map((row) => row.text).join("")).toBe(source);
  for (const row of result.sourceRows)
    expect(/[\uD800-\uDBFF]$/.test(row.text)).toBe(false);
});

test("SVG keeps a complete readable source alternative to the rendered drawing", async () => {
  const source =
    '<svg xmlns="http://www.w3.org/2000/svg"><text>Revenue</text></svg>';
  const result = await prepareHtmlViews(source, () => true, "svg");
  expect(result.rendered?.html).toContain("Revenue");
  expect(result.readingPreview.sourceFallback).toBe(true);
  expect(result.readingPreview.rows.map((row) => row.text).join("")).toBe(
    source,
  );
});

test("portable bundle styles retain pinned data CSS imports and embedded fonts", async () => {
  const imported = Buffer.from(
    "@media(max-width:600px){.report{display:grid}}",
  ).toString("base64");
  const source = `<style>@import url("data:text/css;base64,${imported}") screen;@font-face{font-family:Report;src:url(data:font/woff2;base64,Zm9udA==)}.report{background:url(data:image/png;base64,cGxvdA==)}</style><img src="data:image/png;base64,cGxvdA=="><script>fetch('/private')</script>`;
  const result = await prepareRenderedHtml(source, () => true);
  expect(result.html).toContain(`data:text/css;base64,${imported}`);
  expect(result.html).toContain("data:font/woff2;base64,Zm9udA==");
  expect(result.html).toContain("data:image/png;base64,cGxvdA==");
  expect(result.html).toContain("style-src 'unsafe-inline' data:");
  expect(result.html).toContain("connect-src 'none'");
  expect(result.html).not.toContain("<script>");
});

test("responsive image candidates stay embedded and picture sources remain visual", async () => {
  const source = `<picture><source media="(max-width:600px)" srcset="data:image/png;base64,YQ== 1x, data:image/png;base64,Yg== 2x"><img alt="Plot" src="data:image/png;base64,YQ==" srcset="data:image/png;base64,YQ== 400w, data:image/png;base64,Yg== 800w"></picture><video><source src="https://outside.invalid/video"></video>`;
  const result = await prepareRenderedHtml(source, () => true);
  expect(result.html).toContain("<picture><source media=");
  expect(result.html).toContain("srcset=");
  expect(result.html).toContain("800w");
  expect(result.html).not.toContain("outside.invalid");
  expect(result.html).not.toContain("<video>");
});
test("responsive image candidates cannot smuggle remote or conflicting URLs", async () => {
  for (const value of [
    "data:image/png;base64,YQ== 1x, https://outside.invalid/a 2x",
    "https://outside.invalid/a 1x",
    "data:image/png;base64,YQ== 1x 2x",
    "data:image/png;base64,YQ== 400w 2x",
    "data:image/png;base64,YQ== 0w",
  ]) {
    const result = await prepareRenderedHtml(
      `<img srcset="${value}">`,
      () => true,
    );
    expect(result.html).not.toContain("srcset=");
  }
});

test("SVG image and filter image hrefs retain only embedded raster resources", async () => {
  const { html } = await prepareRenderedHtml(
    '<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image href="data:image/png;base64,AAAA"/><image xlink:href="data:image/jpeg;base64,BBBB"/><filter id="f"><feImage href="data:image/webp;base64,CCCC"/><feImage xlink:href="data:image/gif;base64,DDDD"/></filter><use href="#shape"/></svg>',
    () => true,
  );
  expect(html).toContain('href="data:image/png;base64,AAAA"');
  expect(html).toContain('xlink:href="data:image/jpeg;base64,BBBB"');
  expect(html).toContain('<feImage href="data:image/webp;base64,CCCC">');
  expect(html).toContain('<feImage xlink:href="data:image/gif;base64,DDDD">');
  expect(html).toContain('href="#shape"');
});

test("SVG external images, data SVG/HTML, data use and data links remain inactive", async () => {
  const { html, removed } = await prepareRenderedHtml(
    '<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image href="https://outside.invalid/p.png"/><feImage xlink:href="file:///private"/><image href="data:image/svg+xml;base64,AAAA"/><image href="data:text/html;base64,BBBB"/><use href="data:image/png;base64,CCCC"/><a xlink:href="data:image/png;base64,DDDD">Link</a></svg>',
    () => true,
  );
  expect(removed).toBe(true);
  for (const value of [
    "outside.invalid",
    "file:///private",
    "data:image/svg",
    "data:text/html",
    "base64,CCCC",
    "base64,DDDD",
  ])
    expect(html).not.toContain(value);
});

test("portable SVG images retain nested raster/vector dependencies and responsive candidates", async () => {
  const svg = (text: string) =>
    `data:image/svg+xml;base64,${Buffer.from(text).toString("base64")}`;
  const badge = svg(
    '<svg xmlns="http://www.w3.org/2000/svg"><text>徽章 🚀</text></svg>',
  );
  const plot = svg(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>.plot{fill:green}</style><image href="data:image/png;base64,AAAA"/><image href="${badge}"/><path id="outline" d="M0 0L20 20"/><use href="#outline"/></svg>`,
  );
  const { html, removed } = await prepareRenderedHtml(
    `<picture><source srcset="${plot} 1x, ${badge} 2x"><img alt="Plot" src="${plot}#outline"></picture><svg><image href="${plot}"/><filter><feImage href="${badge}"/></filter></svg>`,
    () => true,
  );
  expect(removed).toBe(false);
  expect(html).toContain(`src="${plot}#outline"`);
  expect(html).toContain(`srcset="${plot} 1x, ${badge} 2x"`);
  expect(html).toContain(`href="${plot}"`);
  expect(html).toContain(`href="${badge}"`);
  expect(html).toContain("script-src 'none'");
});

test("SVG data images refuse malformed active foreign and remote dependency markup", async () => {
  for (const source of [
    "<html/>",
    '<svg xmlns="http://www.w3.org/2000/svg"><g></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><script>run()</script></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" onload="run()"/>',
    '<svg xmlns="http://www.w3.org/2000/svg"><foreignObject><div/></foreignObject></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><animate attributeName="href"/></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" xml:base="https://outside.invalid"/>',
    '<!DOCTYPE svg [<!ENTITY x "expanded">]><svg xmlns="http://www.w3.org/2000/svg">&x;</svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><image href="https://outside.invalid/a.png"/></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><use href="data:image/png;base64,AAAA"/></svg>',
    '<?xml-stylesheet href="remote.css"?><svg xmlns="http://www.w3.org/2000/svg"/>',
  ]) {
    const url = `DATA:IMAGE/SVG+XML;BASE64,${Buffer.from(source).toString("base64")}`;
    const { html, removed } = await prepareRenderedHtml(
      `<img src="${url}" srcset="${url} 1x"><svg><image href="${url}"/><use href="${url}"/><a href="${url}">link</a></svg>`,
      () => true,
    );
    expect(removed).toBe(true);
    expect(html).not.toContain(url);
  }
});

test("bounded SVG decoding preserves native UTF8 and revokes preparation scope", async () => {
  const source = `<svg xmlns="http://www.w3.org/2000/svg"><text>${"a".repeat(8186)}🚀徽章</text></svg>`;
  const url = `data:image/svg+xml;base64,${Buffer.from(source).toString("base64")}`;
  const result = await prepareRenderedHtml(`<img src="${url}">`, () => true);
  expect(result.html).toContain(url);
  let valid = true;
  const pending = prepareRenderedHtml(`<img src="${url}">`, () => valid);
  setTimeout(() => {
    valid = false;
  }, 6);
  await expect(pending).rejects.toThrow("scope changed");
});

test("SVG images reject noncanonical bytes deep XML and excessive nested drawings", async () => {
  const svg = (text: string) =>
    `data:image/svg+xml;base64,${Buffer.from(text).toString("base64")}`;
  let nested = svg('<svg xmlns="http://www.w3.org/2000/svg"/>');
  for (let i = 0; i < 10; i++)
    nested = svg(
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="${nested}"/></svg>`,
    );
  for (const url of [
    "data:image/svg+xml;base64,Zh==",
    `data:image/svg+xml;base64,${Buffer.from([0xc0, 0x80]).toString("base64")}`,
    svg(
      `<svg xmlns="http://www.w3.org/2000/svg">${"<g>".repeat(129)}${"</g>".repeat(129)}</svg>`,
    ),
    nested,
  ]) {
    const { html } = await prepareRenderedHtml(
      `<img src="${url}">`,
      () => true,
    );
    expect(html).not.toContain(url);
  }
});

test("SVG cache shares named views but cannot hide excessive nested depth", async () => {
  const { svgImageValidator } = await import("./svg-images");
  const svg = (text: string) =>
    `data:image/svg+xml;base64,${Buffer.from(text).toString("base64")}`;
  const validate = svgImageValidator(() => true);
  const leaf = svg('<svg xmlns="http://www.w3.org/2000/svg"/>');
  for (let i = 0; i < 40; i++)
    expect(await validate(`${leaf}#view${i}`)).toBe(true);
  let nested = leaf;
  for (let i = 0; i < 8; i++)
    nested = svg(
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="${nested}"/></svg>`,
    );
  expect(await validate(nested)).toBe(true);
  const wrapped = svg(
    `<svg xmlns="http://www.w3.org/2000/svg"><image href="${nested}"/></svg>`,
  );
  expect(await validate(wrapped)).toBe(false);
});

test("SVG Unicode IDs and encoded named views remain local references", async () => {
  const source =
    '<svg xmlns="http://www.w3.org/2000/svg"><path id="σκιά" d="M0 0L1 1"/><use href="#σκιά"/></svg>';
  const url = `data:image/svg+xml;base64,${Buffer.from(source).toString("base64")}#%CF%83%CE%BA%CE%B9%CE%AC`;
  const { html } = await prepareRenderedHtml(
    `<img src="${url}" srcset="${url} 1x">${source}`,
    () => true,
  );
  expect(html).toContain(`src="${url}"`);
  expect(html).toContain(`srcset="${url} 1x"`);
  expect(html).toContain('href="#σκιά"');
});
