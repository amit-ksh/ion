import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_CONTEXT_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
type CssNode = {
  type: string;
  name?: string;
  property?: string;
  prelude?: unknown | null;
  value?: unknown;
};
type CssParser = {
  version: string;
  parse(
    source: string,
    options: { onParseError(error: Error): never },
  ): unknown;
  walk(ast: unknown, visit: (node: CssNode) => void): void;
  generate(ast: unknown): string;
  lexer: {
    matchProperty(property: string, value: unknown): { error: Error | null };
    matchAtrulePrelude(name: string, value: unknown): { error: Error | null };
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
  let uses = 0;
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
      for (const attribute of node.attrs) {
        if (attribute.name === "style")
          styles.push(`.inline{${attribute.value}}`);
        if (
          attribute.name === "id" &&
          /^ion_svg_[a-f0-9]+_badge$/.test(attribute.value)
        )
          prefixes.add(attribute.value.slice(0, -"badge".length));
        if (node.tagName === "use" && attribute.name === "href") {
          expect(attribute.value.startsWith("#ion_svg_")).toBe(true);
          uses++;
        }
      }
    }
    if ("childNodes" in node) pending.push(...node.childNodes);
  }
  for (let index = 0; index < styles.length; index++) {
    expect(styles.length).toBeLessThan(24);
    for (const match of styles[index]!.matchAll(
      /data:text\/css;base64,([a-z0-9+/=]+)/gi,
    ))
      styles.push(Buffer.from(match[1]!, "base64").toString("utf8"));
  }
  expect(uses).toBe(2);
  expect(prefixes.size).toBe(2);
  return { styles, prefixes };
}

test.skipIf(!fixture)(
  "production layer/container identities survive native-to-phone preparation",
  async () => {
    const { styles, prefixes } = await prepared();
    const css = styles.join("\n");
    expect(css).toContain("@layer reset,framework.theme;");
    expect(css).toContain("@container card (width > 1px)");
    for (const prefix of prefixes) {
      expect(css).toContain(
        `layer(${prefix}layer_framework.${prefix}layer_theme)`,
      );
      expect(css).toContain(
        `@layer ${prefix}layer_framework.${prefix}layer_theme`,
      );
      expect(css).toContain(`@container ${prefix}container_card (width >`);
      expect(css).toContain(
        `@container ${prefix}container_card style(--tone:green)`,
      );
      expect(css).toContain(`container:${prefix}container_card/inline-size`);
      expect(css).toContain(`style(container-name:${prefix}container_card)`);
      const imports = styles.filter(
        (style) =>
          style.includes(`opacity:0.7`) &&
          style.includes(`${prefix}layer_framework`),
      );
      expect(imports).toHaveLength(1);
      for (const other of prefixes)
        if (prefix !== other) expect(imports[0]!).not.toContain(other);
    }
    expect(css).toContain("supports(display:grid)");
    expect(css).not.toContain('@import "shared.css"');
  },
);

test.skipIf(!fixture || !parserPath)(
  "independent grammar validates native layer/import/container rules and rejects malformed controls",
  async () => {
    const cssParser = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(Number(cssParser.version.split(".")[0])).toBeGreaterThanOrEqual(3);
    expect(
      cssParser.lexer.matchAtrulePrelude("layer", "base..theme").error,
    ).not.toBeNull();
    expect(
      cssParser.lexer.matchProperty("container-name", "card / size").error,
    ).not.toBeNull();
    const { styles } = await prepared();
    let declarations = 0;
    let rules = 0;
    for (const source of styles) {
      const ast = cssParser.parse(source, {
        onParseError(error) {
          throw error;
        },
      });
      cssParser.walk(ast, (node) => {
        if (
          node.type === "Atrule" &&
          node.name &&
          node.prelude &&
          ["layer", "import", "container"].includes(node.name)
        ) {
          expect(
            cssParser.lexer.matchAtrulePrelude(node.name, node.prelude).error,
          ).toBeNull();
          rules++;
        }
        if (
          node.type === "Declaration" &&
          node.property &&
          ["container", "container-name"].includes(node.property)
        ) {
          expect(
            cssParser.lexer.matchProperty(node.property, node.value).error,
          ).toBeNull();
          declarations++;
        }
      });
    }
    expect(rules).toBeGreaterThanOrEqual(10);
    expect(declarations).toBeGreaterThanOrEqual(6);
  },
  20000,
);
