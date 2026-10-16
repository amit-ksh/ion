import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_ANIMATION_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
type CssNode = {
  type: string;
  name?: string;
  value?: unknown;
  property?: string;
  children?: { toArray(): CssNode[] };
};
type Trace = {
  syntax?: { name?: string; type?: string } | null;
  token?: string;
  match?: Trace[];
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
    matchProperty(
      property: string,
      value: string,
    ): { error: Error | null; matched?: Trace | null };
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
  const styles: string[] = [],
    prefixes = new Set<string>();
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
        ).toBe("Animation Δ");
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
  "native animation shorthand roles and wrappers survive production phone preparation",
  async () => {
    const { styles, prefixes } = await prepared(),
      css = styles.join("\n");
    expect(css).toContain("@keyframes pulse{from{opacity:0.2}to{opacity:1}}");
    expect(css).toContain('animation:1s ident("pulse")');
    for (const prefix of prefixes) {
      for (const value of [
        "@keyframes " + prefix + "animation_linear",
        "@keyframes " + prefix + "animation_both",
        "@keyframes " + prefix + "animation_ready-tail",
        'animation:1s ident("linear") ident("' + prefix + 'animation_linear")',
        '-webkit-animation:2s none ident("' + prefix + 'animation_both")',
        'ident("' +
          prefix +
          'animation_ready-" var(--' +
          prefix +
          "property_suffix))",
        "animation:1s none " + prefix + "animation_both",
        "--" + prefix + 'property_data:ident("pulse")',
        "animation:1s var(--" +
          prefix +
          'property_unknown,linear) ident("linear")',
        "animation:1s linear var(--" +
          prefix +
          'property_unknown,linear) ident("linear")',
        "animation:1s var(--" +
          prefix +
          'property_unknown) ident("' +
          prefix +
          'animation_pulse")',
      ]) {
        expect(css).toContain(value);
      }
      const imported = styles.filter(
        (s) =>
          s.startsWith(".") && s.includes("steps(2,end)") && s.includes(prefix),
      );
      expect(imported).toHaveLength(1);
      for (const other of prefixes)
        if (other !== prefix) expect(imported[0]).not.toContain(other);
    }
  },
);
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
function finite(parser: CssParser, value: CssNode, suffix: string) {
  const copy = parser.parse(parser.generate(value), {
      context: "value",
      onParseError(error) {
        throw error;
      },
    }),
    functions: CssNode[] = [];
  parser.walk(copy, (node) => {
    if (node.type === "Function" && node.name === "ident") functions.push(node);
  });
  for (const fn of functions.reverse()) {
    const values = fn.children!.toArray().map((n) => piece(n, suffix));
    if (!values.length || values.some((v) => v === null) || !values.join(""))
      continue;
    fn.type = "Identifier";
    fn.name = values.join("");
    delete fn.children;
    delete fn.value;
  }
  return parser.generate(copy);
}
function tokens(trace: Trace): string {
  return trace.token ?? trace.match?.map(tokens).join("") ?? "";
}
function roles(trace: Trace | null | undefined, name: string): string[] {
  if (!trace) return [];
  return trace.syntax?.name === name
    ? [tokens(trace)]
    : (trace.match?.flatMap((t) => roles(t, name)) ?? []);
}
test.skipIf(!fixture || !parserPath)(
  "independent shorthand grammar proves timing/fill keywords and private keyframe roles separately",
  async () => {
    const legacy = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(legacy.version).toBe("3.1.0");
    const parser = legacy.fork((config) => {
      config.scope.Value = { ...config.scope.Value };
      delete config.scope.Value.var;
      return config;
    });
    expect(
      parser.lexer.matchProperty("animation", "1s first second").error,
    ).not.toBeNull();
    const none = parser.lexer.matchProperty("animation", "1s none both");
    expect(none.error).toBeNull();
    expect(roles(none.matched, "single-animation-fill-mode")).toEqual(["none"]);
    expect(roles(none.matched, "keyframes-name")).toEqual(["both"]);
    const { styles, prefixes } = await prepared(),
      names = new Set<string>();
    let checked = 0,
      keywordOnly = 0,
      noneFilled = 0,
      wrappers = 0;
    for (const source of styles) {
      const ast = parser.parse(source, {
        parseCustomProperty: true,
        onParseError(error) {
          throw error;
        },
      });
      parser.walk(ast, (node) => {
        if (node.type === "Function" && node.name === "ident") wrappers++;
        if (
          node.type !== "Declaration" ||
          !["animation", "-webkit-animation"].includes(node.property ?? "")
        )
          return;
        for (const suffix of ["tail", "none", "9", "é"]) {
          const value = finite(parser, node.value as CssNode, suffix);
          if (/\b(var|ident)\(/.test(value)) continue;
          const match = parser.lexer.matchProperty("animation", value);
          expect(match.error).toBeNull();
          const keyframes = roles(match.matched, "keyframes-name");
          keyframes.forEach((name) => names.add(name));
          if (value === "1s linear") {
            expect(keyframes).toEqual([]);
            keywordOnly++;
          }
          if (/^1s none ion_svg_/.test(value)) {
            expect(roles(match.matched, "single-animation-fill-mode")).toEqual([
              "none",
            ]);
            expect(keyframes).toHaveLength(1);
            noneFilled++;
          }
          checked++;
        }
      });
    }
    expect(checked).toBeGreaterThan(30);
    expect(keywordOnly).toBeGreaterThan(0);
    expect(noneFilled).toBeGreaterThan(0);
    expect(wrappers).toBeGreaterThan(15);
    expect(names.has("pulse")).toBe(true);
    for (const prefix of prefixes) {
      for (const name of ["linear", "both"])
        expect(names.has(prefix + "animation_" + name)).toBe(true);
      for (const suffix of ["tail", "none", "9", "é"])
        expect(names.has(prefix + "animation_ready-" + suffix)).toBe(true);
    }
  },
);
