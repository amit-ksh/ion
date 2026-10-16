import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";
const fixture = process.env.ION_ANIMATION_FALLBACK_FIXTURE,
  parserPath = process.env.ION_CSS_PARSER;
type Node = {
  type: string;
  name?: string;
  value?: unknown;
  property?: string;
  children?: { toArray(): Node[] };
};
type Trace = {
  syntax?: { name?: string } | null;
  token?: string;
  match?: Trace[];
};
type Css = {
  version: string;
  fork(
    extension: (config: {
      scope: { Value: Record<string, unknown> };
    }) => unknown,
  ): Css;
  parse(
    source: string,
    options?: {
      context?: string;
      parseCustomProperty?: boolean;
      onParseError?(error: Error): never;
    },
  ): Node;
  generate(node: Node): string;
  walk(node: Node, visit: (node: Node) => void): void;
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
  const pending = [parser.document as DefaultTreeAdapterMap["node"]],
    styles: string[] = [],
    prefixes = new Set<string>();
  let texts = 0,
    uses = 0;
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
        ).toBe("Fallback Δ");
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
  expect(texts).toBe(2);
  expect(uses).toBe(2);
  return { styles, prefixes };
}
test.skipIf(!fixture)(
  "native whole-member animation fallbacks remain private and partial fragments stay in context",
  async () => {
    const { styles, prefixes } = await prepared(),
      css = styles.join("\n");
    expect(css).toContain("animation:var(--outer,1s pulse)");
    for (const p of prefixes) {
      for (const value of [
        "animation:var(--" +
          p +
          "property_missing,1s linear " +
          p +
          "animation_pulse)",
        "-webkit-animation:var(--" +
          p +
          "property_missing,var(--" +
          p +
          "property_other,2s none " +
          p +
          "animation_both))",
        "animation:var(--" +
          p +
          "property_multi,1s " +
          p +
          "animation_pulse,2s linear " +
          p +
          "animation_linear),var(--" +
          p +
          "property_last,3s none " +
          p +
          "animation_both)",
        "animation:var(--" +
          p +
          "property_present,1s " +
          p +
          "animation_pulse)",
        "animation:var(--" + p + "property_empty,)",
        "animation:1s var(--" + p + 'property_unknown,linear) ident("linear")',
        "animation:var(--" + p + "property_unknown,linear) 1s linear",
        "--" + p + "property_data:var(--" + p + "property_missing,1s pulse)",
        'animation:var(ident("--' +
          p +
          'property_computed"),1s ident("' +
          p +
          'animation_pulse"))',
      ])
        expect(css).toContain(value);
      const imported = styles.filter(
        (s) =>
          s.startsWith(".") && s.includes("property_computed") && s.includes(p),
      );
      expect(imported).toHaveLength(1);
      for (const other of prefixes)
        if (other !== p) expect(imported[0]).not.toContain(other);
    }
  },
);
function identifier(node: Node): string | null {
  if (node.type === "Identifier") return node.name!;
  if (node.type === "String") return String(node.value);
  if (node.type === "Number" && /^[+-]?\d+$/.test(String(node.value)))
    return String(Number(node.value));
  if (node.type === "Function" && node.name === "ident") {
    const parts = node.children!.toArray().map(identifier);
    return parts.length && parts.every((p) => p !== null) && parts.join("")
      ? parts.join("")
      : null;
  }
  return null;
}
/** Finite missing/present/empty input contract, not a cascade or cycle engine.
 * The native output retains var()/ident(); only test values are substituted. */
function finite(
  css: Css,
  nodes: Node[],
  values: Map<string, string>,
  depth = 0,
): string | null {
  if (depth > 32) throw new Error("Test value nesting");
  const parts: string[] = [];
  for (const node of nodes) {
    if (node.type === "Function" && node.name === "var") {
      const args = node.children!.toArray(),
        comma = args.findIndex((n) => n.type === "Operator" && n.value === ",");
      const first = comma < 0 ? args : args.slice(0, comma),
        name = first.length === 1 ? identifier(first[0]!) : null;
      if (!name) return null;
      if (values.has(name)) {
        parts.push(values.get(name)!);
        continue;
      }
      if (comma < 0) return null;
      const fallback = finite(css, args.slice(comma + 1), values, depth + 1);
      if (fallback === null) return null;
      parts.push(fallback);
    } else if (node.type === "Function" && node.name === "ident") {
      const name = identifier(node);
      if (name === null) return null;
      parts.push(css.generate({ type: "Identifier", name }));
    } else if (node.type === "Function") {
      const body = finite(css, node.children!.toArray(), values, depth + 1);
      if (body === null) return null;
      parts.push(node.name + "(" + body + ")");
    } else parts.push(css.generate(node));
  }
  return parts.join(" ");
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
  "independent fallback selection and shorthand roles retain missing, present and valid-empty distinctions",
  async () => {
    const legacy = createRequire(import.meta.url)(parserPath!) as Css;
    expect(legacy.version).toBe("3.1.0");
    const css = legacy.fork((config) => {
      config.scope.Value = { ...config.scope.Value };
      delete config.scope.Value.var;
      return config;
    });
    expect(
      css.lexer.matchProperty("animation", "1s first second").error,
    ).not.toBeNull();
    expect(css.lexer.matchProperty("animation", "").error).not.toBeNull();
    const { styles, prefixes } = await prepared(),
      names = new Set<string>();
    const present = new Map<string, string>(),
      empty = new Map<string, string>();
    for (const p of prefixes) {
      present.set("--" + p + "property_present", "0s none");
      empty.set("--" + p + "property_present", "");
    }
    let checked = 0,
      presentSelections = 0,
      emptySelections = 0,
      wrappers = 0;
    for (const source of styles) {
      const ast = css.parse(source, {
        parseCustomProperty: true,
        onParseError(error) {
          throw error;
        },
      });
      css.walk(ast, (node) => {
        if (node.type === "Function" && node.name === "var") wrappers++;
        if (
          node.type !== "Declaration" ||
          !["animation", "-webkit-animation"].includes(node.property ?? "")
        )
          return;
        const value = node.value as Node,
          raw = css.generate(value);
        // Partial shorthand fragments have deliberately unproven keyword roles.
        if (raw.includes("property_unknown")) return;
        for (const values of [new Map<string, string>(), present, empty]) {
          const rendered = finite(css, value.children!.toArray(), values);
          if (rendered === null) continue;
          const match = css.lexer.matchProperty("animation", rendered);
          if (!rendered.trim()) {
            expect(match.error).not.toBeNull();
            if (raw.includes("property_present") && values === empty)
              emptySelections++;
            continue;
          }
          expect(match.error).toBeNull();
          const keyframes = roles(match.matched, "keyframes-name");
          keyframes.forEach((name) => names.add(name));
          if (raw.includes("property_present") && values === present) {
            expect(keyframes).toEqual([]);
            presentSelections++;
          }
          checked++;
        }
      });
    }
    expect(checked).toBeGreaterThan(30);
    expect(wrappers).toBeGreaterThan(15);
    expect(presentSelections).toBeGreaterThan(0);
    expect(emptySelections).toBeGreaterThan(0);
    expect(names.has("pulse")).toBe(true);
    for (const p of prefixes)
      for (const name of ["pulse", "linear", "both"])
        expect(names.has(p + "animation_" + name)).toBe(true);
  },
);
