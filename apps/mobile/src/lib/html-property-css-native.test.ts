import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_PROPERTY_CSS_FIXTURE;
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
    options: {
      parseCustomProperty?: boolean;
      onParseError(error: Error): never;
    },
  ): unknown;
  walk(ast: unknown, visit: (node: CssNode) => void): void;
  generate(ast: unknown): string;
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
    prefixes = new Set<string>(),
    presentations: string[] = [];
  let uses = 0,
    paths = 0,
    labels = 0;
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
        if (["fill", "opacity", "font-family"].includes(attribute.name))
          presentations.push(attribute.value);
        if (attribute.name === "d") {
          expect(attribute.value).toBe("M0 0L20 20");
          paths++;
        }
        if (attribute.name === "data-label") {
          expect(attribute.value).toBe("var(--Tone)");
          labels++;
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
  expect(paths).toBe(2);
  expect(labels).toBe(2);
  return { styles, prefixes, presentations };
}
test.skipIf(!fixture)(
  "production custom-property identities remain distinct through phone preparation",
  async () => {
    const { styles, prefixes, presentations } = await prepared();
    const css = styles.join("\n");
    expect(css).toContain('syntax:"<color>"');
    expect(css).toContain('syntax:"<number>"');
    expect(css).toContain(
      '@property --Tone{syntax:"*";inherits:true;initial-value:black}',
    );
    expect(css).toContain("color:var(--Tone)");
    for (const prefix of prefixes) {
      expect(css).toContain(`@property --${prefix}property_Tone`);
      expect(css).toContain(`@property --${prefix}property_opacity`);
      expect(css).toContain(
        `--${prefix}property_tone:var(--${prefix}property_Tone)`,
      );
      expect(css).toContain(
        `var(--${prefix}property_missing,var(--${prefix}property_Tone))`,
      );
      expect(css).toContain(
        `--${prefix}property_cycle:var(--${prefix}property_cycle)`,
      );
      expect(css).toContain(`transition-property:--${prefix}property_Tone`);
      expect(css).toContain(`will-change:--${prefix}property_opacity`);
      expect(css).toContain(`style(--${prefix}property_Tone:green)`);
      expect(css).toContain(`--${prefix}property_text:"--Tone"`);
      expect(css).toContain(`--${prefix}property_data:--Tone`);
      expect(presentations).toContain(`var(--${prefix}property_Tone)`);
      expect(presentations).toContain(`var(--${prefix}property_opacity)`);
      expect(presentations).toContain(`var(--${prefix}property_family,serif)`);
      const imports = styles.filter((style) =>
        style.startsWith(`@property --${prefix}property_opacity`),
      );
      expect(imports).toHaveLength(1);
      for (const other of prefixes)
        if (other !== prefix)
          expect(imports[0]).not.toContain(`--${other}property_`);
    }
  },
);
test.skipIf(!fixture || !parserPath)(
  "independent CSS Tree parser validates native registration and property-reference grammar",
  async () => {
    const parser = createRequire(import.meta.url)(parserPath!) as CssParser;
    expect(Number(parser.version.split(".")[0])).toBeGreaterThanOrEqual(3);
    expect(
      parser.lexer.matchAtrulePrelude("property", '"not-a-property"').error,
    ).not.toBeNull();
    expect(
      parser.lexer.matchAtruleDescriptor("property", "inherits", "sometimes")
        .error,
    ).not.toBeNull();
    const { styles, presentations } = await prepared();
    let registrations = 0,
      descriptors = 0,
      references = 0;
    for (const source of styles) {
      const ast = parser.parse(source, {
        parseCustomProperty: true,
        onParseError(error) {
          throw error;
        },
      });
      parser.walk(ast, (node) => {
        if (
          node.type === "Atrule" &&
          node.name?.toLowerCase() === "property" &&
          node.prelude
        ) {
          expect(
            parser.lexer.matchAtrulePrelude("property", node.prelude).error,
          ).toBeNull();
          registrations++;
        }
        if (
          node.type === "Declaration" &&
          node.property &&
          ["syntax", "inherits", "initial-value"].includes(node.property)
        ) {
          expect(
            parser.lexer.matchAtruleDescriptor(
              "property",
              node.property,
              node.value,
            ).error,
          ).toBeNull();
          descriptors++;
        }
        if (
          node.type === "Declaration" &&
          node.property &&
          ["transition-property", "will-change"].includes(node.property)
        ) {
          expect(
            parser.lexer.matchProperty(node.property, node.value).error,
          ).toBeNull();
          references++;
        }
      });
    }
    // CSS Tree deliberately refuses computed-value matching with var(). Check
    // its independent syntax tree instead; substitution and typing belong to
    // the browser, not to this fixture or the portable document serializer.
    let variables = 0;
    for (const value of presentations) {
      const ast = parser.parse(`.presentation{fill:${value}}`, {
        onParseError(error) {
          throw error;
        },
      });
      parser.walk(ast, (node) => {
        if (node.type === "Function" && node.name?.toLowerCase() === "var") {
          expect(parser.generate(node)).toMatch(
            /^var\(--ion_svg_[a-f0-9]+_property_[A-Za-z]+(?:,serif)?\)$/,
          );
          variables++;
        }
      });
    }
    expect(variables).toBeGreaterThanOrEqual(6);
    expect(registrations).toBeGreaterThanOrEqual(5);
    expect(descriptors).toBeGreaterThanOrEqual(15);
    expect(references).toBeGreaterThanOrEqual(4);
  },
  20000,
);
