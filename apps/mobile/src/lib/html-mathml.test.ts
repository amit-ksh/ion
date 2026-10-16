import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import {
  parse,
  html,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareHtmlViews, prepareRenderedHtml } from "./html-rendering";

type Element = DefaultTreeAdapterMap["element"];
function elements(source: string): Element[] {
  const pending: DefaultTreeAdapterMap["node"][] = [
    parse(source, { scriptingEnabled: false }),
  ];
  const result: Element[] = [];
  while (pending.length) {
    const node = pending.pop()!;
    if (tree.isElementNode(node)) result.push(node);
    if ("childNodes" in node) pending.push(...node.childNodes);
  }
  return result;
}
function attr(node: Element, name: string) {
  return node.attrs.find((entry) => entry.name === name)?.value;
}
function expectInactive(source: string) {
  for (const node of elements(source)) {
    expect(
      [
        "script",
        "iframe",
        "foreignObject",
        "object",
        "embed",
        "mglyph",
        "malignmark",
        "maction",
      ].includes(node.tagName),
    ).toBe(false);
    for (const entry of node.attrs) {
      expect(entry.name.startsWith("on")).toBe(false);
      expect(
        [
          "srcdoc",
          "href",
          "src",
          "action",
          "altimg",
          "autofocus",
          "nonce",
          "is",
        ].includes(entry.name),
      ).toBe(false);
    }
  }
  expect(source).toContain("script-src 'none'");
  expect(source).toContain("connect-src 'none'");
  expect(source).toContain("frame-src 'none'");
}

test("equations retain fractions, radicals, scripts, matrices and native MathML namespaces", async () => {
  const source = `<h1>Model report</h1><math display="block" aria-label="Quadratic roots"><mrow><mi>x</mi><mo>=</mo><mfrac><mrow><mo>&minus;</mo><mi>b</mi><mo>&PlusMinus;</mo><msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>&minus;</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt></mrow><mrow><mn>2</mn><mi>a</mi></mrow></mfrac></mrow></math><math><munder><mo>&sum;</mo><mi>i</mi></munder><msubsup><mi>x</mi><mi>i</mi><mn>2</mn></msubsup><mroot><mi>y</mi><mn>3</mn></mroot><mtable><mtr><mtd><mn>1</mn></mtd><mtd><mn>0</mn></mtd></mtr></mtable><mmultiscripts><mi>T</mi><none/><mi>j</mi><mprescripts/><mi>i</mi><none/></mmultiscripts></math>`;
  const result = await prepareRenderedHtml(source, () => true);
  expect(result.removed).toBe(false);
  const nodes = elements(result.html),
    mathNodes = nodes.filter((node) => node.namespaceURI === html.NS.MATHML);
  expect(mathNodes.length).toBeGreaterThan(40);
  for (const tag of [
    "mfrac",
    "msqrt",
    "mroot",
    "msup",
    "msubsup",
    "munder",
    "mtable",
    "mtr",
    "mtd",
    "mmultiscripts",
    "mprescripts",
    "none",
  ])
    expect(mathNodes.some((node) => node.tagName === tag)).toBe(true);
  expect(
    attr(
      mathNodes.find((node) => attr(node, "aria-label"))!,
      "aria-label",
    ),
  ).toBe("Quadratic roots");
  for (const character of ["−", "±", "∑"])
    expect(result.html).toContain(character);
  expectInactive(result.html);
});

test("math layout CSS, hidden spacing and accessible author labels stay authored", async () => {
  const source = `<style>math {font-size:1.2rem;math-style:normal} .tight {math-depth:2; color:#123}</style><math display="block" aria-label="Energy" style="math-style:compact"><mstyle class="tight" displaystyle="true" scriptlevel="1"><mi mathvariant="normal">E</mi><mo>=</mo><mpadded width="+1em" voffset=".1em"><mphantom><mi>x</mi></mphantom></mpadded><mspace width="1em"/><mover accent="true"><mi>m</mi><mo>^</mo></mover><msup><mi>c</mi><mn>2</mn></msup></mstyle></math>`;
  const result = await prepareRenderedHtml(source, () => true);
  expect(result.removed).toBe(false);
  for (const part of [
    "math-depth:2",
    'aria-label="Energy"',
    'displaystyle="true"',
    'scriptlevel="1"',
    'mathvariant="normal"',
    "<mphantom>",
    'width="+1em"',
    'accent="true"',
  ])
    expect(result.html).toContain(part);
  expectInactive(result.html);
});

test("trusted text and XML annotations retain native HTML/SVG/math integration with active content removed", async () => {
  const source = `<math><semantics><mi>x</mi><annotation encoding="application/x-tex">x &lt; y</annotation><annotation-xml encoding="TEXT/HTML"><div style="color:green"><strong>Explanation</strong><script>private()</script><svg><text>Diagram</text><foreignObject><iframe src="https://outside.invalid"></iframe></foreignObject></svg><math><mfrac><mn>1</mn><mn>2</mn></mfrac></math></div></annotation-xml><annotation-xml encoding="application/mathml-presentation+xml"><math><msup><mi>x</mi><mn>2</mn></msup></math></annotation-xml></semantics></math>`;
  const result = await prepareRenderedHtml(source, () => true);
  expect(result.removed).toBe(true);
  const nodes = elements(result.html);
  expect(nodes.find((node) => node.tagName === "strong")?.namespaceURI).toBe(
    html.NS.HTML,
  );
  expect(nodes.find((node) => node.tagName === "text")?.namespaceURI).toBe(
    html.NS.SVG,
  );
  expect(
    nodes.filter((node) => node.tagName === "mfrac")[0]?.namespaceURI,
  ).toBe(html.NS.MATHML);
  expect(
    nodes.filter((node) => node.tagName === "annotation-xml"),
  ).toHaveLength(2);
  expect(result.html).toContain("x &lt; y");
  expect(result.html).not.toContain("private()");
  expect(result.html).not.toContain("outside.invalid");
  expectInactive(result.html);
});

test("unknown, external and ambiguous annotations and glyph resources remain inactive", async () => {
  for (const child of [
    `<annotation>Untrusted marker</annotation>`,
    `<annotation encoding="text/custom">Untrusted marker</annotation>`,
    `<annotation encoding="text/plain"><mi>Untrusted marker</mi></annotation>`,
    `<annotation encoding="application/x-tex" href="https://outside.invalid">Untrusted marker</annotation>`,
    `<annotation-xml encoding="text/html" xlink:href="file:///private">Untrusted marker</annotation-xml>`,
    `<annotation-xml encoding="image/svg+xml"><svg><text>Untrusted marker</text></svg></annotation-xml>`,
    `<mglyph src="https://outside.invalid" alt="Untrusted marker"/>`,
  ]) {
    const result = await prepareRenderedHtml(
      `<math xmlns:xlink="http://www.w3.org/1999/xlink"><semantics><mi>x</mi>${child}</semantics></math>`,
      () => true,
    );
    expect(result.removed).toBe(true);
    expect(result.html).toContain("<mi>x</mi>");
    expect(result.html).not.toContain("Untrusted marker");
    expect(result.html).not.toContain("outside.invalid");
    expectInactive(result.html);
  }
  const result = await prepareRenderedHtml(
    `<math altimg="https://outside.invalid" href="ion://run" onclick="run()" autofocus nonce="secret"><mi definitionURL="file:///private">x</mi></math>`,
    () => true,
  );
  expect(result.html).not.toContain("outside.invalid");
  expect(result.html).not.toContain("file:///private");
  expectInactive(result.html);
});

test("interactive math actions become static rows with all their content", async () => {
  const result = await prepareRenderedHtml(
    `<math><maction id="choice" actiontype="toggle" selection="2"><mi>x</mi><mi>y</mi></maction></math>`,
    () => true,
  );
  const node = elements(result.html).find(
    (entry) => attr(entry, "id") === "choice",
  )!;
  expect(result.removed).toBe(true);
  expect(node.tagName).toBe("mrow");
  expect(node.namespaceURI).toBe(html.NS.MATHML);
  expect(
    node.childNodes.filter(tree.isElementNode).map((entry) => entry.tagName),
  ).toEqual(["mi", "mi"]);
  expect(attr(node, "selection")).toBeUndefined();
  expect(attr(node, "actiontype")).toBeUndefined();
  expectInactive(result.html);
});

test("foreign-content reparsing cannot restore active math/HTML mutation payloads", async () => {
  for (const input of [
    `<math><mtext><table><mglyph><style><!--</style><img title="--><img src=x onerror=alert(1)>">`,
    `<math><mtext><table><malignmark><style><!--</style><img title="--><img src=x onerror=alert(1)>">`,
    `<math><annotation-xml encoding="text/html"><svg><style><img src=x onerror=alert(1)></style></svg></annotation-xml></math>`,
    `<math><mtext><a href="javascript:run()"><img src="file:///private" onerror="run()"></a><iframe srcdoc="<script>run()</script>"></iframe></mtext></math>`,
    `<math><annotation-xml encoding="application/xhtml+xml"><form action="ion://run"><button onclick="run()">Run</button></form></annotation-xml></math>`,
  ]) {
    let source = input;
    for (let i = 0; i < 3; i++) {
      source = (await prepareRenderedHtml(source, () => true)).html;
      expectInactive(source);
    }
  }
});

test("source remains exact and Reading names unsupported math instead of flattening it", async () => {
  const source = `<p>Equation:</p><math aria-label="A fraction"><mfrac><mi>x</mi><mi>y</mi></mfrac></math>`;
  const result = await prepareHtmlViews(source, () => true);
  expect(result.sourceRows.map((row) => row.text).join("")).toBe(source);
  expect(result.rendered?.html).toContain("<mfrac>");
  expect(JSON.stringify(result.readingPreview)).toContain("Math not displayed");
  expect(result.renderError).toBeNull();
});

test("math work stays cancellable and uses existing tree complexity bounds", async () => {
  let checks = 0;
  await expect(
    prepareRenderedHtml(
      `<math>${"<mrow><mi>x</mi><mo>+</mo></mrow>".repeat(1200)}</math>`,
      () => ++checks < 6,
    ),
  ).rejects.toThrow("scope changed");
  await expect(
    prepareRenderedHtml(
      `<math>${"<mrow>".repeat(130)}<mi>x</mi>${"</mrow>".repeat(130)}</math>`,
      () => true,
    ),
  ).rejects.toThrow("too complex");
});

const fixture = process.env.ION_MATHML_NATIVE_FIXTURE;
test.skipIf(!fixture)(
  "actual native publication, sealed replay and phone preparation preserve technical equations",
  async () => {
    const value = JSON.parse(readFileSync(fixture!, "utf8")) as {
      html: string;
      sha256: string;
      source: string;
      summary: string;
    };
    expect(createHash("sha256").update(value.html).digest("hex")).toBe(
      value.sha256,
    );
    expect(value.summary).toContain("equations");
    expect(value.html).toContain("data:text/css;base64,");
    const result = await prepareHtmlViews(value.html, () => true);
    expect(result.sourceRows.map((row) => row.text).join("")).toBe(value.html);
    expect(result.rendered?.removed).toBe(false);
    const nodes = elements(result.rendered!.html);
    expect(nodes.find((node) => node.tagName === "math")?.namespaceURI).toBe(
      html.NS.MATHML,
    );
    expect(result.rendered!.html).toContain("<mfrac>");
    expect(result.rendered!.html).toContain("<msqrt>");
    expect(result.rendered!.html).toContain('aria-label="Quadratic roots"');
    expect(result.rendered!.html).toContain("∑");
    expect(value.source).toContain("&sum;");
    expectInactive(result.rendered!.html);
  },
);
