import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_COMPUTED_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
type CssNode = {
  type: string;
  name?: string;
  value?: string;
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
  walk(ast: CssNode, callback: (node: CssNode) => void): void;
  lexer: {
    matchProperty(name: string, value: string): { error: Error | null };
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
    presentations: string[] = [];
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
        ).toBe("Report 🚀");
        texts++;
      }
      for (const attr of node.attrs) {
        if (attr.name === "id" && /^ion_svg_[a-f0-9]+_badge$/.test(attr.value))
          prefixes.add(attr.value.slice(0, -5));
        if (node.tagName === "use" && attr.name === "href") {
          expect(attr.value.startsWith("#ion_svg_")).toBe(true);
          uses++;
        }
        if (attr.name === "font-family") presentations.push(attr.value);
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
  return { styles, presentations, prefixes };
}
test.skipIf(!fixture)(
  "native computed CSS identities survive phone preparation across two imported documents",
  async () => {
    const { styles, presentations, prefixes } = await prepared();
    const css = styles.join("\n");
    expect(css).toContain('animation-name:ident("pulse")');
    expect(css).toContain('font-family:ident("Report"),serif');
    for (const prefix of prefixes) {
      for (const value of [
        `@keyframes ${prefix}animation_pulse`,
        `ident("${prefix}animation_pulse")`,
        `ident("${prefix}animation_pulse-" var(--${prefix}property_suffix))`,
        `var(--${prefix}property_missing,${prefix}animation_pulse)`,
        `container-name:ident("${prefix}container_card-" var(--${prefix}property_suffix))`,
        `transition-property:ident("--${prefix}property_Tone")`,
        `fill:var(ident("--${prefix}property_Tone"),green)`,
        `font-family:ident("${prefix}font_report"),serif`,
        `font-palette:ident("--${prefix}font_palette_Cool")`,
        `styleset(ident("${prefix}font_feature_Loop"))`,
        `--${prefix}property_text:ident("pulse")`,
        `--${prefix}property_opaque:ident("Report")`,
        `animation-name:ident("none"),ident("n" var(--${prefix}property_suffix))`,
      ])
        expect(css).toContain(value);
      expect(presentations).toContain(`ident("${prefix}font_report")`);
      const imported = styles.filter(
        (s) => s.startsWith("@font-face{font-family:") && s.includes(prefix),
      );
      expect(imported).toHaveLength(1);
      expect(imported[0]).toContain(
        `font-family:var(--${prefix}property_unknown,"${prefix}font_report"),serif`,
      );
      for (const other of prefixes)
        if (other !== prefix) expect(imported[0]).not.toContain(other);
    }
  },
);
// Finite identifier contract evaluation over an independent AST. This does not
// emulate the cascade, inherited variables or a phone's computed style engine.
function identifier(node: CssNode, suffix: string): string | null {
  if (node.type === "Identifier") return node.name!;
  if (node.type === "String") return node.value!;
  if (node.type === "Number" && /^[+-]?\d+$/.test(node.value!))
    return String(Number(node.value));
  if (node.type === "Function" && node.name === "var") {
    const args = node.children!.toArray();
    return args.length === 1 &&
      args[0]?.type === "Identifier" &&
      /^--ion_svg_[a-f0-9]+_property_suffix$/.test(args[0].name!)
      ? suffix
      : null;
  }
  return null;
}
test.skipIf(!fixture || !parserPath)(
  "independent CSS AST retains wrappers and resolves only the finite computed identifier contract",
  async () => {
    const legacy = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(legacy.version).toBe("3.1.0");
    // CSS Tree 3.1.0 enforces Variables 1's static first argument. Extend only
    // its Value-scope var recognizer to generic AST parsing for Variables 2;
    // explicit computed-name shape checks remain below. No package is patched.
    expect(() =>
      legacy.parse('var(ident("--tone"),red)', {
        context: "value",
        onParseError(error) {
          throw error;
        },
      }),
    ).toThrow();
    const parser = legacy.fork((config) => {
      config.scope.Value = { ...config.scope.Value };
      delete config.scope.Value.var;
      return config;
    });
    for (const invalid of ["", "a,b", "5px", "rgb(0 0 0)"]) {
      const ast = parser.parse(`ident(${invalid})`, { context: "value" });
      const children = ast.children!.toArray()[0]!.children!.toArray();
      expect(
        children.length > 0 &&
          children.every((n) => identifier(n, "tail") !== null),
      ).toBe(false);
    }
    const { styles, prefixes } = await prepared();
    const generated = new Set<string>();
    let calls = 0;
    for (const source of styles) {
      const ast = parser.parse(source, {
        parseCustomProperty: true,
        onParseError(error) {
          throw error;
        },
      });
      parser.walk(ast, (node) => {
        if (
          node.type === "Function" &&
          node.name?.toLowerCase() === "var" &&
          node.children?.toArray()[0]?.type === "Function"
        ) {
          const args = node.children.toArray();
          expect(args[0]?.name).toBe("ident");
          expect(args[1]?.type).toBe("Operator");
          expect(args[1]?.value).toBe(",");
          const pieces = args[0]!
            .children!.toArray()
            .map((part) => identifier(part, "tail"));
          expect(pieces.every((piece) => piece !== null)).toBe(true);
          expect(pieces.join("")).toMatch(
            /^--ion_svg_[a-f0-9]+_property_Tone$/,
          );
        }
        if (node.type !== "Function" || node.name?.toLowerCase() !== "ident")
          return;
        calls++;
        for (const suffix of ["tail", "none", "", "9", "é"]) {
          const pieces = node
            .children!.toArray()
            .map((part) => identifier(part, suffix));
          if (pieces.some((part) => part === null)) continue;
          generated.add(pieces.join(""));
        }
      });
    }
    expect(calls).toBeGreaterThan(20);
    for (const prefix of prefixes) {
      for (const name of [
        `${prefix}animation_pulse`,
        `${prefix}font_report`,
        `--${prefix}font_palette_Cool`,
        `${prefix}font_feature_Loop`,
        `--${prefix}property_Tone`,
      ])
        expect(generated.has(name)).toBe(true);
      for (const suffix of ["tail", "none", "", "9", "é"])
        for (const name of [
          `${prefix}animation_pulse-${suffix}`,
          `${prefix}container_card-${suffix}`,
          `--${prefix}font_palette_Cool-${suffix}`,
          `${prefix}font_feature_Loop-${suffix}`,
        ])
          expect(generated.has(name)).toBe(true);
      for (const property of [
        "animation-name",
        "container-name",
        "font-family",
      ]) {
        const name =
          property === "font-family"
            ? `${prefix}font_report`
            : property === "container-name"
              ? `${prefix}container_card-tail`
              : `${prefix}animation_pulse-tail`;
        expect(parser.lexer.matchProperty(property, name).error).toBeNull();
      }
    }
    expect(generated.has("none")).toBe(true);
    expect(generated.has("nnone")).toBe(true);
  },
);
