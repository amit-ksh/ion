import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_COUNTER_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
type CssNode = {
  type: string;
  name?: string;
  value?: unknown;
  property?: string;
  children?: { toArray(): CssNode[] };
};
type CssParser = {
  version: string;
  fork(
    extension: (config: {
      scope: { Value: Record<string, unknown> };
    }) => unknown,
  ): CssParser;
  parse(
    source: string,
    options?: {
      context?: string;
      parseCustomProperty?: boolean;
      onParseError?(error: Error): never;
    },
  ): CssNode;
  generate(node: CssNode): string;
  walk(node: CssNode, visit: (node: CssNode) => void): void;
  lexer: {
    matchProperty(property: string, value: string): { error: Error | null };
  };
};

async function prepared() {
  const result = await prepareRenderedHtml(
    readFileSync(fixture!, "utf8"),
    () => true,
  );
  expect(result.removed).toBe(false);
  expect(result.html).toContain("script-src 'none'");
  const parser = new Parser<DefaultTreeAdapterMap>({ scriptingEnabled: false });
  parser.tokenizer.write(result.html, true);
  const pending = [parser.document as DefaultTreeAdapterMap["node"]];
  const styles: string[] = [];
  const prefixes = new Set<string>();
  let uses = 0,
    texts = 0;
  while (pending.length) {
    const node = pending.pop()!;
    if (tree.isElementNode(node)) {
      if (node.tagName === "style")
        styles.push(
          node.childNodes
            .filter(tree.isTextNode)
            .map((n) => n.value)
            .join(""),
        );
      if (node.tagName === "text") {
        expect(
          node.childNodes
            .filter(tree.isTextNode)
            .map((n) => n.value)
            .join(""),
        ).toBe("Counter Δ 7");
        texts++;
      }
      for (const attr of node.attrs) {
        if (attr.name === "style") styles.push(".inline{" + attr.value + "}");
        if (attr.name === "id" && /^ion_svg_[a-f0-9]+_badge$/.test(attr.value))
          prefixes.add(attr.value.slice(0, -5));
        if (node.tagName === "use" && attr.name === "href") {
          expect(attr.value.startsWith("#ion_svg_")).toBe(true);
          uses++;
        }
      }
    }
    if ("childNodes" in node) pending.push(...node.childNodes);
  }
  for (let i = 0; i < styles.length; i++) {
    expect(styles.length).toBeLessThan(16);
    for (const match of styles[i]!.matchAll(
      /data:text\/css;base64,([a-z0-9+/=]+)/gi,
    ))
      styles.push(Buffer.from(match[1]!, "base64").toString("utf8"));
  }
  expect(prefixes.size).toBe(2);
  expect(uses).toBe(2);
  expect(texts).toBe(2);
  return { styles, prefixes };
}

test.skipIf(!fixture)(
  "native counter styles stay private through production phone preparation",
  async () => {
    const { styles, prefixes } = await prepared();
    const css = styles.join("\n");
    expect(css).toContain('@counter-style marks{system:cyclic;symbols:"O"}');
    expect(css).toMatch(
      /\.owner:not\(:where\(\[data-ion-svg-import-[a-f0-9]{64}\]\)\)\{list-style-type:ident\("marks"\)\}/,
    );
    for (const prefix of prefixes) {
      for (const value of [
        "@counter-style " + prefix + "counter_style_marks",
        "@counter-style " +
          prefix +
          "counter_style_mark-tail{system:extends " +
          prefix +
          "counter_style_marks}",
        "@counter-style " + prefix + "counter_style_7",
        "@counter-style " +
          prefix +
          "counter_style_lower-roman{system:extends lower-roman}",
        'list-style-type:ident("' + prefix + 'counter_style_marks")',
        'list-style-type:ident("' + prefix + 'counter_style_7")',
        "list-style:inside var(--" +
          prefix +
          'property_unknown,ident("' +
          prefix +
          'counter_style_marks"))',
        'counter(step,ident("' +
          prefix +
          'counter_style_mark-" var(--' +
          prefix +
          "property_suffix)))",
        'counters(chapter,".",var(--' +
          prefix +
          "property_unknown," +
          prefix +
          "counter_style_marks))",
        "counter(var(--" +
          prefix +
          "property_step,marks),var(--" +
          prefix +
          'property_unknown,ident("' +
          prefix +
          'counter_style_marks")))',
        'list-style-type:ident("lower-" var(--' + prefix + "property_suffix))",
        "--" + prefix + 'property_opaque:ident("marks")',
        "--" +
          prefix +
          'property_caption:counter(step,ident("' +
          prefix +
          'counter_style_marks"))',
      ])
        expect(css).toContain(value);
      const imported = styles.filter(
        (s) =>
          s.startsWith("@counter-style ") &&
          s.includes(prefix) &&
          s.includes('symbols:"A"'),
      );
      expect(imported).toHaveLength(1);
      expect(imported[0]).toContain(
        "@media(min-width:999999px){@counter-style " +
          prefix +
          "counter_style_lower-roman",
      );
      for (const other of prefixes)
        if (other !== prefix) expect(imported[0]).not.toContain(other);
    }
  },
);

// Evaluate a finite test contract only. Production retains every ident()/var()
// wrapper. This does not implement a cascade, custom-property inheritance,
// fallback selection, counter rendering, or browser support detection.
function piece(node: CssNode, suffix: string): string | null {
  if (node.type === "Identifier") return node.name!;
  if (node.type === "String") return String(node.value);
  if (node.type === "Number" && /^[+-]?\d+$/.test(String(node.value)))
    return String(Number(node.value));
  if (node.type === "Function" && node.name === "var") {
    const args = node.children!.toArray();
    if (
      args.length === 1 &&
      args[0]?.type === "Identifier" &&
      /^--ion_svg_[a-f0-9]+_property_suffix$/.test(args[0].name!)
    )
      return suffix;
  }
  return null;
}
function finiteValue(parser: CssParser, node: CssNode, suffix: string) {
  const copy = parser.parse(parser.generate(node), {
      context: "value",
      onParseError(error) {
        throw error;
      },
    }),
    functions: CssNode[] = [];
  parser.walk(copy, (n) => {
    if (n.type === "Function" && n.name === "ident") functions.push(n);
  });
  for (const fn of functions.reverse()) {
    const pieces = fn.children!.toArray().map((n) => piece(n, suffix));
    if (!pieces.length || pieces.some((p) => p === null) || !pieces.join(""))
      continue;
    fn.type = "Identifier";
    fn.name = pieces.join("");
    delete fn.children;
    delete fn.value;
  }
  return parser.generate(copy);
}

test.skipIf(!fixture || !parserPath)(
  "independent counter grammar checks finite computed results without consuming fallback wrappers",
  async () => {
    const legacy = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(legacy.version).toBe("3.1.0");
    const parser = legacy.fork((config) => {
      config.scope.Value = { ...config.scope.Value };
      delete config.scope.Value.var;
      return config;
    });
    expect(
      parser.lexer.matchProperty("content", "counters(section)").error,
    ).not.toBeNull();
    expect(
      parser.lexer.matchProperty("list-style-type", "3px").error,
    ).not.toBeNull();
    const { styles, prefixes } = await prepared();
    const generated = new Set<string>();
    let checked = 0,
      calls = 0,
      variables = 0;
    for (const source of styles) {
      const ast = parser.parse(source, {
        parseCustomProperty: true,
        onParseError(error) {
          throw error;
        },
      });
      parser.walk(ast, (node) => {
        if (node.type === "Function" && node.name === "ident") {
          calls++;
          for (const suffix of ["tail", "roman", "none", "", "9", "é"]) {
            const pieces = node
              .children!.toArray()
              .map((n) => piece(n, suffix));
            if (pieces.length && pieces.every((p) => p !== null))
              generated.add(pieces.join(""));
          }
        }
        if (node.type === "Function" && node.name === "var") {
          const args = node.children!.toArray();
          expect(args[0]?.type).toBe("Identifier");
          expect(args[0]?.name).toMatch(
            /^--ion_svg_[a-f0-9]+_property_(unknown|step|suffix)$/,
          );
          if (args.length > 1) {
            expect(args[1]?.type).toBe("Operator");
            expect(args[1]?.value).toBe(",");
          }
          variables++;
        }
        const property =
          node.type === "Declaration" &&
          /^(list-style(?:-type)?|content)$/.test(node.property ?? "")
            ? node.property
            : undefined;
        const counter =
          node.type === "Function" &&
          /^(counter|counters)$/.test(node.name ?? "");
        if (!property && !counter) return;
        for (const suffix of ["tail", "roman", "none", "", "9", "é"]) {
          const value = finiteValue(
            parser,
            property ? (node.value as CssNode) : node,
            suffix,
          );
          if (/\b(?:var|ident)\(/.test(value)) continue;
          expect(
            parser.lexer.matchProperty(property ?? "content", value).error,
          ).toBeNull();
          checked++;
        }
      });
    }
    expect(calls).toBeGreaterThan(15);
    expect(variables).toBeGreaterThan(8);
    expect(checked).toBeGreaterThan(30);
    for (const prefix of prefixes) {
      for (const name of ["marks", "7", "lower-roman"])
        expect(generated.has(prefix + "counter_style_" + name)).toBe(true);
      for (const suffix of ["tail", "roman", "none", "", "9", "é"])
        expect(generated.has(prefix + "counter_style_mark-" + suffix)).toBe(
          true,
        );
    }
    expect(generated.has("marks")).toBe(true);
    expect(generated.has("lower-roman")).toBe(true);
    expect(generated.has("none")).toBe(true);
  },
);
