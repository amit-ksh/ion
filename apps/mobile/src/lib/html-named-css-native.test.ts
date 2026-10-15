import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_NAMED_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
type CssNode = { type: string; property?: string; value?: unknown };
type CssParser = {
  version: string;
  parse(
    source: string,
    options: { onParseError: (error: Error) => never },
  ): unknown;
  walk(ast: unknown, visit: (node: CssNode) => void): void;
  lexer: {
    matchProperty(property: string, value: unknown): { error: Error | null };
  };
};
test.skipIf(!fixture)(
  "production named CSS keeps two sprite definitions separate through phone preparation",
  async () => {
    const source = readFileSync(fixture!, "utf8");
    const result = await prepareRenderedHtml(source, () => true);
    expect(result.removed).toBe(false);
    const parser = new Parser<DefaultTreeAdapterMap>({
      scriptingEnabled: false,
    });
    parser.tokenizer.write(result.html, true);
    const pending = [parser.document as DefaultTreeAdapterMap["node"]];
    const styles: string[] = [];
    let local = 0;
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
        for (const attr of node.attrs) {
          if (attr.name === "style") styles.push(`.inline{${attr.value}}`);
          if (attr.name === "href") {
            expect(attr.value.startsWith("#")).toBe(true);
            local++;
          }
        }
      }
      if ("childNodes" in node) pending.push(...node.childNodes);
    }
    expect(local).toBe(2);
    for (const style of [...styles]) {
      for (const match of style.matchAll(
        /data:text\/css;base64,([a-z0-9+/=]+)/gi,
      ))
        styles.push(Buffer.from(match[1]!, "base64").toString("utf8"));
    }
    const css = styles.join("\n");
    const definitions = [
      ...css.matchAll(/@keyframes\s+(ion_svg_[a-f0-9_]+animation_pulse)/g),
    ].map((m) => m[1]!);
    const unique = [...new Set(definitions)];
    expect(unique).toHaveLength(2);
    for (const name of unique) {
      expect(css).toContain(`animation-name:${name}`);
      expect(css).toContain(
        `@counter-style ${name.replace("animation_pulse", "counter_style_marks")}`,
      );
    }
    expect(css).toContain("@keyframes pulse{from{opacity:1}to{opacity:0.2}}");
    expect(css).not.toContain("from:where");
    expect(css).not.toContain("50%:where");
    expect(result.html).toContain("script-src 'none'");
  },
);

test.skipIf(!fixture || !parserPath)(
  "native CSS remains grammatical after phone preparation using an independent CSS parser",
  async () => {
    const cssParser = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(Number(cssParser.version.split(".")[0])).toBeGreaterThanOrEqual(3);
    // Prove the independent value grammar rejects a malformed counter call.
    expect(
      cssParser.lexer.matchProperty("content", "counters(section)").error,
    ).not.toBeNull();
    const result = await prepareRenderedHtml(
      readFileSync(fixture!, "utf8"),
      () => true,
    );
    const parser = new Parser<DefaultTreeAdapterMap>({
      scriptingEnabled: false,
    });
    parser.tokenizer.write(result.html, true);
    const pending = [parser.document as DefaultTreeAdapterMap["node"]];
    const styles: string[] = [];
    while (pending.length) {
      const node = pending.pop()!;
      if (tree.isElementNode(node) && node.tagName === "style")
        styles.push(
          node.childNodes
            .filter(tree.isTextNode)
            .map((n) => n.value)
            .join(""),
        );
      if ("childNodes" in node) pending.push(...node.childNodes);
    }
    for (let i = 0; i < styles.length; i++)
      for (const match of styles[i]!.matchAll(
        /data:text\/css;base64,([a-z0-9+/=]+)/gi,
      ))
        styles.push(Buffer.from(match[1]!, "base64").toString("utf8"));
    let checked = 0;
    for (const css of styles) {
      const ast = cssParser.parse(css, {
        onParseError(error) {
          throw error;
        },
      });
      cssParser.walk(ast, (node) => {
        if (
          node.type === "Declaration" &&
          node.property &&
          /^(animation(?:-name)?|list-style(?:-type)?|content)$/.test(
            node.property,
          )
        ) {
          expect(
            cssParser.lexer.matchProperty(node.property, node.value).error,
          ).toBeNull();
          checked++;
        }
      });
    }
    expect(checked).toBeGreaterThanOrEqual(8);
  },
);
