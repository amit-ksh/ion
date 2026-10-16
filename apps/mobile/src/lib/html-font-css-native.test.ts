import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_FONT_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
type CssNode = {
  type: string;
  name?: string;
  property?: string;
  prelude?: unknown;
  value?: unknown;
};
type CssParser = {
  version: string;
  parse(
    source: string,
    options: { onParseError(error: Error): never },
  ): unknown;
  walk(ast: unknown, visit: (node: CssNode) => void): void;
  lexer: {
    matchAtrulePrelude(name: string, value: unknown): { error: Error | null };
    matchAtruleDescriptor(
      rule: string,
      name: string,
      value: unknown,
    ): { error: Error | null };
    matchProperty(name: string, value: unknown): { error: Error | null };
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
          prefixes.add(attr.value.slice(0, -"badge".length));
        if (node.tagName === "use" && attr.name === "href") {
          expect(attr.value.startsWith("#ion_svg_")).toBe(true);
          uses++;
        }
        if (["font-family", "font-variant"].includes(attr.name))
          presentations.push(attr.value);
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
  return { styles, prefixes, presentations };
}
test.skipIf(!fixture)(
  "production font palettes and feature keys remain distinct through phone preparation",
  async () => {
    const { styles, prefixes, presentations } = await prepared();
    const css = styles.join("\n");
    expect(css).toContain(
      "@font-palette-values --Cool{font-family:Report;base-palette:0}",
    );
    expect(css).toContain("font-variant-alternates:styleset(Loop)");
    for (const prefix of prefixes) {
      expect(css).toContain(
        `@font-palette-values --${prefix}font_palette_Cool`,
      );
      expect(css).toContain(
        `@font-feature-values"${prefix}font_report",Installed Face`,
      );
      expect(css).toContain(
        `styleset(${prefix}font_feature_Loop,${prefix}font_feature_loop) swash(${prefix}font_feature_Loop)`,
      );
      expect(css).toContain(`@annotation{${prefix}font_feature_Note:2}`);
      expect(css).toContain(`annotation(${prefix}font_feature_Note)`);
      expect(css).toContain(
        `@supports(font-palette:--${prefix}font_palette_Cool)`,
      );
      expect(presentations).toContain(`"${prefix}font_report",serif`);
      expect(presentations).toContain(`styleset(${prefix}font_feature_Loop)`);
      const imported = styles.filter((style) =>
        style.startsWith(`@font-face{font-family:"${prefix}font_report"`),
      );
      expect(imported).toHaveLength(1);
      expect(imported[0]).toContain("data:font/woff2;base64,");
      for (const other of prefixes)
        if (other !== prefix) expect(imported[0]).not.toContain(other);
    }
    expect(css).toContain("base-palette:1;override-colors:0 red,1 blue");
    expect(css).toContain("base-palette:1;override-colors:0 green,1 black");
    expect(css).toContain(
      'font-variation-settings:"wght" 600;font-feature-settings:"swsh" 1',
    );
  },
);
test.skipIf(!fixture || !parserPath)(
  "independent CSS Tree validates native font rule and property grammar",
  async () => {
    const css = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(Number(css.version.split(".")[0])).toBeGreaterThanOrEqual(3);
    expect(
      css.lexer.matchAtrulePrelude("font-palette-values", "plain-name").error,
    ).not.toBeNull();
    expect(
      css.lexer.matchAtruleDescriptor(
        "font-palette-values",
        "base-palette",
        "-2",
      ).error,
    ).not.toBeNull();
    expect(
      css.lexer.matchProperty("font-variant-alternates", "styleset(1)").error,
    ).not.toBeNull();
    const { styles } = await prepared();
    let palettes = 0,
      features = 0,
      descriptors = 0,
      properties = 0;
    for (const source of styles) {
      const ast = css.parse(source, {
        onParseError(error) {
          throw error;
        },
      });
      css.walk(ast, (node) => {
        if (
          node.type === "Atrule" &&
          node.prelude &&
          ["font-palette-values", "font-feature-values"].includes(
            node.name ?? "",
          )
        ) {
          expect(
            css.lexer.matchAtrulePrelude(node.name!, node.prelude).error,
          ).toBeNull();
          if (node.name === "font-palette-values") palettes++;
          else features++;
        }
        if (
          node.type === "Declaration" &&
          node.property &&
          ["base-palette", "override-colors"].includes(node.property)
        ) {
          expect(
            css.lexer.matchAtruleDescriptor(
              "font-palette-values",
              node.property,
              node.value,
            ).error,
          ).toBeNull();
          descriptors++;
        }
        if (
          node.type === "Declaration" &&
          node.property &&
          [
            "font-family",
            "font-palette",
            "font-variant",
            "font-variant-alternates",
            "font-variation-settings",
            "font-feature-settings",
          ].includes(node.property)
        ) {
          expect(
            css.lexer.matchProperty(node.property, node.value).error,
          ).toBeNull();
          properties++;
        }
      });
    }
    expect(palettes).toBe(3);
    expect(features).toBe(4);
    expect(descriptors).toBe(5);
    expect(properties).toBeGreaterThanOrEqual(21);
  },
  20000,
);
