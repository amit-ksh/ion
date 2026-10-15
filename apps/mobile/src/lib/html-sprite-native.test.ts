import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_SVG_SPRITE_FIXTURE;
test.skipIf(!fixture)(
  "production native sprite output retains complete local references and scoped styles on phones",
  async () => {
    const source = readFileSync(fixture!, "utf8");
    const result = await prepareRenderedHtml(source, () => true);
    expect(result.removed).toBe(false);
    const parser = new Parser<DefaultTreeAdapterMap>({
      scriptingEnabled: false,
    });
    parser.tokenizer.write(result.html, true);
    const ids = new Set<string>(),
      refs: string[] = [],
      styles: string[] = [];
    const pending = [parser.document as DefaultTreeAdapterMap["node"]];
    let symbols = 0;
    while (pending.length) {
      const node = pending.pop()!;
      if (tree.isElementNode(node)) {
        if (node.tagName === "symbol") symbols++;
        for (const attr of node.attrs) {
          if (attr.name === "id") {
            expect(ids.has(attr.value)).toBe(false);
            ids.add(attr.value);
          }
          if (attr.name === "href") {
            expect(attr.value.startsWith("#")).toBe(true);
            refs.push(attr.value.slice(1));
          }
        }
        if (node.tagName === "style")
          styles.push(
            node.childNodes
              .filter(tree.isTextNode)
              .map((child) => child.value)
              .join(""),
          );
      }
      if ("childNodes" in node) pending.push(...node.childNodes);
    }
    expect(symbols).toBe(2);
    expect(refs.length).toBeGreaterThanOrEqual(4);
    for (const id of refs) expect(ids.has(id)).toBe(true);
    expect(
      styles.some((style) => style.includes(":where([data-ion_svg_")),
    ).toBe(true);
    expect(
      styles.some((style) => style.includes("data:text/css;base64,")),
    ).toBe(true);
    expect(result.html).toContain("徽章 🚀");
    expect(result.html).toContain("Along route");
    expect(result.html).toContain("script-src 'none'");
    expect(result.html).not.toContain('href="icons/');
    expect(result.html).not.toContain("::before:where");
  },
);
