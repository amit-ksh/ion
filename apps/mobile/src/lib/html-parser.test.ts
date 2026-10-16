import { expect, test } from "bun:test";
import { parse, serialize, defaultTreeAdapter as tree } from "parse5";
import { documentHtmlParser, HTML_PARSE_CHUNK } from "./html-parser";
function prepared(source: string, batch = HTML_PARSE_CHUNK) {
  const parser = documentHtmlParser();
  for (let i = 0; i < source.length; i += batch)
    parser.write(source.slice(i, i + batch), i + batch >= source.length);
  if (!source.length) parser.write("", true);
  return parser.document;
}
test("bounded HTML5 parsing matches stock tree construction across arbitrary chunk boundaries", () => {
  const cases = [
    "",
    "<!doctype html><p>語🚀 &amp; &#x1f680; \r\n next</p>",
    "<textarea>\nfirst &amp; second</textarea><pre>\r\nhello</pre>",
    '<style>p{content:"&amp; <!-- </stylX>"}</style><p>end',
    "<table>before<tr><td><b>cell</table>after</b>",
    "<p><b>one<i>two</b>three</i><div>four",
    "<svg><g><![CDATA[<path> & ]]></g><text>語🚀</text></svg>",
    '<math><mtext><table><mglyph><style><!--</style><img title="--><img src=x onerror=x>">',
    '<noscript><p title="</noscript><img src=x onerror=x>">',
    '<p title="one&amp;two" data-a=some&#32;text hidden>x</p>',
    "<!-- one --!> <p>two<!--three--><!-- four -->end",
    "<p>\0x&#0; &#xD800; &#x110000; &notin= &notit;</p>",
    "<select><option>one<table><option>two</select>",
    "<head><title>abc &amp; end</title><body>tail",
    "<template><table><tr><td>content</template>after",
    '<p title="' + "x".repeat(24000) + '&amp;end" data-b="tail">text</p>',
    "<!--" + "text".repeat(7000) + "--><p>tail</p>",
    '<p title="' + "x".repeat(18000) + '">before</p><span ',
    '<p title="' +
      "x".repeat(18000) +
      '" title="duplicate">before</p><span hidden',
  ];
  for (const source of cases) {
    const expected = serialize(parse(source, { scriptingEnabled: false }));
    for (const batch of [1, 7, 31, 8191, 8192])
      expect(serialize(prepared(source, batch))).toBe(expected);
  }
});
test("large character runs have bounded text nodes and exact visible text", () => {
  const text = "x".repeat(4 * 1024 * 1024);
  const document = prepared("<p>" + text + "</p>");
  const pending = [document as import("parse5").DefaultTreeAdapterMap["node"]];
  const parts: string[] = [];
  while (pending.length) {
    const node = pending.pop()!;
    if (tree.isTextNode(node)) {
      expect(node.value.length).toBeLessThanOrEqual(HTML_PARSE_CHUNK);
      parts.push(node.value);
    }
    if ("childNodes" in node)
      for (let i = node.childNodes.length - 1; i >= 0; i--)
        pending.push(node.childNodes[i]!);
  }
  expect(parts.join("")).toBe(text);
  expect(parts.length).toBeGreaterThan(500);
}, 20000);
test("token and element complexity refuse while normal bounded attributes remain supported", () => {
  for (const source of [
    "<" + "x".repeat(9000) + ">",
    "<p " + "x".repeat(9000) + '="v">',
    '<!doctype html PUBLIC "' + "x".repeat(18000) + '">',
    "<div>".repeat(150) + "end" + "</div>".repeat(150),
    "<p " +
      Array.from({ length: 520 }, (_, i) => `data-a${i}="x"`).join(" ") +
      ">",
  ])
    expect(() => prepared(source)).toThrow("too complex");
});

test("entity offsets retain exact text after input-buffer compaction", () => {
  for (const [source, expected] of [
    ["&".repeat(200000), "&".repeat(200000)],
    ["&#65;".repeat(40000), "A".repeat(40000)],
    ["&#x1f680;".repeat(24000), "🚀".repeat(24000)],
    ["&notin;".repeat(30000), "∉".repeat(30000)],
  ]) {
    const document = prepared(`<p>${source}</p>`);
    const pending = [
      document as import("parse5").DefaultTreeAdapterMap["node"],
    ];
    const actual: string[] = [];
    while (pending.length) {
      const node = pending.pop()!;
      if (tree.isTextNode(node)) actual.push(node.value);
      if ("childNodes" in node)
        for (let i = node.childNodes.length - 1; i >= 0; i--)
          pending.push(node.childNodes[i]!);
    }
    expect(actual.join("") === expected).toBe(true);
  }
});
