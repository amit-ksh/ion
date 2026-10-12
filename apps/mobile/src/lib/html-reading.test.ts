import { expect, test } from "bun:test";
import {
  documentPreviewRows,
  documentReadingPreview,
  prepareDocumentReading,
} from "./document-preview";
import { htmlNodeText, readHtml } from "./html-reading";
const visible = (html: string) =>
  readHtml(html).nodes.map(htmlNodeText).join("\n");

test("complete HTML and fragments preserve common semantic structure using only native Markdown nodes", () => {
  const html = `<!DOCTYPE html><html><head><title>Private title</title><style>body {color:red}</style></head><body>
<h1>Report</h1><p>Hello <strong>team</strong>, <em>today</em> <a href="https://secret.example/private">open reference</a>.</p>
<ol start="3"><li>First</li><li>Second<ul><li>Nested</li></ul></li></ol>
<blockquote><p>Evidence</p></blockquote><pre><code>let a = 1;\nreturn a;</code></pre><hr>
<table><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody><tr><td>cost</td><td>3</td></tr></tbody></table></body></html>`;
  const nodes = readHtml(html).nodes;
  expect(nodes.map((node) => node.type)).toEqual([
    "heading",
    "paragraph",
    "list",
    "list",
    "blockquote",
    "code",
    "thematicBreak",
    "table",
  ]);
  expect(nodes[2]).toMatchObject({ type: "list", ordered: true, start: 3 });
  expect(nodes[3]).toMatchObject({ type: "list", ordered: true, start: 4 });
  const json = JSON.stringify(nodes);
  expect(json).toContain('"type":"strong"');
  expect(json).toContain('"type":"emphasis"');
  expect(json).not.toContain("Private title");
  expect(json).not.toContain("secret.example");
  expect(json).not.toContain('"type":"link"');
  expect(json).not.toContain('"type":"html"');
  expect(visible("<p>Fragment</p>")).toBe("Fragment");
});

test("scripts, styling, browser attributes and external resources never enter the derived native tree", () => {
  const html = `<h2 style="background:url(https://secret.test)" onclick="execute()">Safe</h2>
<script type="module">const x = '<img src=x>'; fetch('https://secret.test');</script><STYLE>@import 'https://secret.test';</STYLE>
<p><a href="javascript:execute()" ping="https://secret.test">Label</a><img src="https://secret.test" onerror="execute()" alt="Diagram"></p>
<iframe src="https://secret.test">Hidden</iframe><form action="https://secret.test"><input value="Private"><button>Submit</button></form>
<svg><image href="https://secret.test" /></svg><object data="https://secret.test">Private</object><video src="https://secret.test">Private</video>`;
  const json = JSON.stringify(readHtml(html));
  for (const forbidden of [
    "https://secret.test",
    "execute()",
    "Hidden",
    "Private",
    "Submit",
    '"type":"image"',
    '"url":',
  ])
    expect(json).not.toContain(forbidden);
  expect(json).toContain("Safe");
  expect(json).toContain("Label");
  expect(json).toContain("Image not displayed: Diagram");
  for (const kind of [
    "Frame",
    "Form",
    "Vector image",
    "Embedded object",
    "Video",
  ])
    expect(json).toContain(`${kind} not displayed`);
});

test("entity decoding is single pass, preserves Unicode, and cannot introduce active markup", () => {
  const html =
    "<p>नमस्ते &amp; 語 &#128512; &#x1f642; &lt;script&gt; &amp;lt;img&amp;gt; &unknown; &#0;</p>";
  const text = visible(html);
  expect(text).toBe("नमस्ते & 語 😀 🙂 <script> &lt;img&gt; &unknown; �");
  expect(readHtml(html).nodes[0]?.type).toBe("paragraph");
  expect(visible("<p>1 &LT; 2 &AMP; 3</p>")).toBe("1 < 2 & 3");
});

test("quoted attribute delimiters, comments and raw-text closing tags do not corrupt visible text", () => {
  expect(
    visible(
      `<p title="a > b">A<!-- <img src=x> --> <b>B</b></p><script>'<p>'</ScRiPt><p>C</p>`,
    ),
  ).toBe("A B\nC");
  expect(visible("<p>1 < 2</p>")).toBe("1 < 2");
  expect(visible("<pre>a &lt; b\n  two</pre>")).toBe("a < b\n  two");
});

test("malformed, deeply nested, ambiguous or overly complex HTML returns complete source with an explicit fallback note", () => {
  for (const html of [
    "<p>unclosed",
    "<p>x</div>",
    '<p title="x>oops</p>',
    "<!--never closed",
    "<script>never closed",
    "<table><tr>not a cell</tr></table>",
    '<p alt="x" ALT="y">ambiguous</p>',
    "<div>".repeat(33) + "x" + "</div>".repeat(33),
    "<br>".repeat(1025),
    "<!doctype html>".repeat(4097),
  ]) {
    // Inputs outside the complete-text limit are rejected, not returned partly.
    if (html.length > 65_536) {
      expect(() => documentReadingPreview("html", html)).toThrow("limit");
      continue;
    }
    const result = documentReadingPreview("html", html);
    expect(result.sourceFallback).toBe(true);
    expect(result.note).toContain("Showing complete Source");
    expect(
      result.rows.every((row) => row.node === null && row.text.length <= 1000),
    ).toBe(true);
    expect(result.rows.map((row) => row.text).join("")).toBe(html);
  }
});

test("large paragraphs and long lists remain bounded virtualized rows with all visible text retained", () => {
  const text = "語".repeat(15_000),
    paragraph = documentReadingPreview("html", `<p>${text}</p>`);
  expect(paragraph.sourceFallback).toBe(false);
  expect(paragraph.rows.length).toBeGreaterThan(10);
  expect(paragraph.rows.every((row) => row.text.length <= 1000)).toBe(true);
  expect(paragraph.rows.map((row) => row.text).join("")).toBe(text);
  const html =
    "<ul>" +
    Array.from({ length: 150 }, (_, i) => `<li>Item ${i}</li>`).join("") +
    "</ul>";
  const rows = documentReadingPreview("html", html).rows;
  expect(rows).toHaveLength(150);
  expect(rows.at(-1)?.text).toBe("Item 149");
});

test("tables chunk rows, bound columns and repeat the column labels for each native section", () => {
  const html =
    "<table><tr><th>A</th><th>B</th></tr>" +
    Array.from(
      { length: 20 },
      (_, i) => `<tr><td>${i}</td><td>x</td></tr>`,
    ).join("") +
    "</table>";
  const result = documentReadingPreview("htm", html);
  expect(result.sourceFallback).toBe(false);
  expect(result.rows).toHaveLength(3);
  for (const row of result.rows)
    expect(row.text.startsWith("A\tB\n")).toBe(true);
  const wide = "<table><tr>" + "<td>x</td>".repeat(17) + "</tr></table>";
  expect(documentReadingPreview("html", wide).sourceFallback).toBe(true);
});

test("reading expansion and node density cannot defeat complete text and per-row rendering limits", () => {
  const dense = `<p>${"<b>x</b>".repeat(200)}</p>`;
  const rows = documentReadingPreview("html", dense).rows;
  expect(rows[0].node).toMatchObject({
    type: "paragraph",
    children: [{ type: "text", value: "x".repeat(200) }],
  });
  const expanded = "<p>" + "x".repeat(50_000) + "</p>" + "<img>".repeat(800);
  const result = documentReadingPreview("html", expanded);
  expect(result.sourceFallback).toBe(true);
  expect(result.rows.map((row) => row.text).join("")).toBe(expanded);
});

test("Source is exact, parsing does not require browser TextEncoder, and fallback never truncates authored bytes", () => {
  const original = globalThis.TextEncoder,
    html = "<p>नमस्ते <b>語</b></p>";
  try {
    Object.assign(globalThis, { TextEncoder: undefined });
    expect(documentReadingPreview("html", html).rows[0]?.text).toBe(
      "नमस्ते 語",
    );
    expect(
      documentPreviewRows("html", html, false)
        .map((row) => row.text)
        .join(""),
    ).toBe(html);
  } finally {
    Object.assign(globalThis, { TextEncoder: original });
  }
  expect(() => documentReadingPreview("html", "語".repeat(22_000))).toThrow(
    "limit",
  );
});

test("cached preparation yields between HTML batches and refuses changed owner/foreground generations", async () => {
  let live = true,
    calls = 0;
  const html = "<ul>" + "<li>Item</li>".repeat(100) + "</ul>";
  const result = await prepareDocumentReading("html", html, () => {
    calls++;
    return live;
  });
  expect(calls).toBeGreaterThan(4);
  expect(result.rows).toHaveLength(100);
  setTimeout(() => {
    live = false;
  }, 0);
  await expect(
    prepareDocumentReading("html", html, () => live),
  ).rejects.toThrow("scope changed");
  await expect(
    prepareDocumentReading("html", "<p>unclosed", () => false),
  ).rejects.toThrow("scope changed");
});
