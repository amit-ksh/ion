import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Parser,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";
import { prepareRenderedHtml } from "./html-rendering";

const fixture = process.env.ION_SCOPE_CSS_FIXTURE;
const parserPath = process.env.ION_CSS_PARSER;
const selectorPath = process.env.ION_SELECTOR_ENGINE;
type DomNode = {
  name?: string;
  data?: string;
  attribs?: Record<string, string>;
  children?: DomNode[];
  parent: DomNode | null;
};
type Scope = { type: "Scope"; root: unknown | null; limit: unknown | null };
type CssParser = {
  version: string;
  parse(
    source: string,
    options: { onParseError: (error: Error) => never },
  ): unknown;
  walk(
    ast: unknown,
    visit: (node: {
      type: string;
      name?: string;
      prelude?: unknown | null;
    }) => void,
  ): void;
  generate(ast: unknown): string;
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
  const styles: { css: string; owner: DefaultTreeAdapterMap["node"] }[] = [];
  while (pending.length) {
    const node = pending.pop()!;
    if (tree.isElementNode(node) && node.tagName === "style") {
      styles.push({
        css: node.childNodes
          .filter(tree.isTextNode)
          .map((n) => n.value)
          .join(""),
        owner: node.parentNode!,
      });
    }
    if ("childNodes" in node) pending.push(...node.childNodes);
  }
  for (let i = 0; i < styles.length; i++) {
    expect(styles.length).toBeLessThan(16);
    const { css, owner } = styles[i]!;
    for (const match of css.matchAll(/data:text\/css;base64,([a-z0-9+/=]+)/gi))
      styles.push({
        css: Buffer.from(match[1]!, "base64").toString("utf8"),
        owner,
      });
  }
  const cssParser = createRequire(import.meta.url)(parserPath!) as CssParser;
  expect(Number(cssParser.version.split(".")[0])).toBeGreaterThanOrEqual(3);
  const scopes: { scope: Scope; owner: DefaultTreeAdapterMap["node"] }[] = [];
  for (const { css, owner } of styles) {
    const ast = cssParser.parse(css, {
      onParseError(error) {
        throw error;
      },
    });
    cssParser.walk(ast, (node) => {
      if (node.type === "Scope") scopes.push({ scope: node as Scope, owner });
      // A completely implicit scope has no prelude, so CSS Tree represents
      // it as an Atrule with null prelude instead of a Scope prelude node.
      else if (
        node.type === "Atrule" &&
        node.name?.toLowerCase() === "scope" &&
        node.prelude === null
      )
        scopes.push({
          scope: { type: "Scope", root: null, limit: null },
          owner,
        });
    });
  }
  return { parser, scopes, cssParser, result };
}

test.skipIf(!fixture || !parserPath)(
  "production CSS scopes survive phone preparation with valid native selector grammar",
  async () => {
    const { scopes, cssParser, result } = await prepared();
    expect(() =>
      cssParser.parse("@scope (???) {path{fill:red}}", {
        onParseError(error) {
          throw error;
        },
      }),
    ).toThrow();
    expect(scopes.length).toBeGreaterThanOrEqual(11);
    expect(scopes.filter(({ scope }) => scope.root === null)).toHaveLength(4);
    const roots = scopes
      .filter(({ scope }) => scope.root !== null)
      .map(({ scope }) => cssParser.generate(scope.root));
    expect(roots.filter((root) => root.startsWith("#ion_svg_"))).toHaveLength(
      2,
    );
    expect(roots.filter((root) => root.startsWith(":scope"))).toHaveLength(2);
    expect(roots.filter((root) => root.includes("_root]"))).toHaveLength(2);
    expect(result.html).not.toContain('href="a.svg');
    expect(result.html).not.toContain('href="b.svg');
  },
);

test.skipIf(!fixture || !parserPath || !selectorPath)(
  "independent selector engine binds scope roots and limits only to their original sprite after phone preparation",
  async () => {
    const { parser, scopes, cssParser } = await prepared();
    const require = createRequire(selectorPath!);
    const { Document, Element, Text } = require("domhandler") as {
      Document: new (children: DomNode[]) => DomNode;
      Element: new (
        name: string,
        attrs: Record<string, string>,
        children: DomNode[],
      ) => DomNode;
      Text: new (data: string) => DomNode;
    };
    const selectors = createRequire(import.meta.url)(selectorPath!) as {
      selectAll(
        query: string,
        nodes: DomNode,
        options: { xmlMode: true; context?: DomNode[] },
      ): DomNode[];
    };
    const converted = new Map<DefaultTreeAdapterMap["node"], DomNode>();
    function convert(
      node: DefaultTreeAdapterMap["node"],
      parent: DomNode | null,
    ): DomNode {
      const dom = tree.isElementNode(node)
        ? new Element(
            node.tagName,
            Object.fromEntries(node.attrs.map((a) => [a.name, a.value])),
            [],
          )
        : tree.isTextNode(node)
          ? new Text(node.value)
          : new Document([]);
      dom.parent = parent;
      converted.set(node, dom);
      if ("childNodes" in node)
        dom.children = node.childNodes.map((n) => convert(n, dom));
      return dom;
    }
    const dom = convert(parser.document, null);
    const query = (value: string, context?: DomNode) =>
      selectors.selectAll(value, dom, {
        xmlMode: true,
        ...(context ? { context: [context] } : {}),
      });
    const outer = scopes.filter(
      ({ scope }) =>
        scope.root && cssParser.generate(scope.root).startsWith("#ion_svg_"),
    );
    const ids = new Set<string>();
    for (const { scope, owner } of outer) {
      const roots = query(cssParser.generate(scope.root));
      expect(roots).toHaveLength(1);
      const panel = roots[0]!;
      expect(panel.attribs?.["data-label"]).toBe("panel");
      expect(panel.parent).toBe(converted.get(owner)!);
      const id = panel.attribs!.id!;
      expect(ids.has(id)).toBe(false);
      ids.add(id);
      const limits = query(cssParser.generate(scope.limit), panel);
      expect(limits).toHaveLength(1);
      expect(limits[0]!.attribs?.["data-label"]).toBe("stop");
      expect(limits[0]!.parent).toBe(panel);
      const nested = scopes.find(
        (entry) =>
          entry.owner === owner &&
          entry.scope.root &&
          cssParser.generate(entry.scope.root).startsWith(":scope"),
      )!;
      const inner = query(cssParser.generate(nested.scope.root), panel);
      expect(inner).toHaveLength(1);
      expect(inner[0]!.attribs?.["data-label"]).toBe("inner");
      const innerLimits = query(
        cssParser.generate(nested.scope.limit),
        inner[0],
      );
      expect(innerLimits).toHaveLength(1);
      expect(innerLimits[0]!.attribs?.["data-label"]).toBe("limit");
      expect(innerLimits[0]!.parent).toBe(inner[0]);
      const implicit = scopes.find(
        (entry) =>
          entry.scope.root === null &&
          entry.scope.limit === null &&
          converted.get(entry.owner) === inner[0],
      );
      expect(implicit).toBeDefined();
      const svgScope = scopes.find(
        (entry) =>
          entry.owner === owner &&
          entry.scope.root &&
          cssParser.generate(entry.scope.root).includes("_root]"),
      )!;
      expect(query(cssParser.generate(svgScope.scope.root))).toEqual([
        converted.get(owner)!,
      ]);
      const stops = query(
        cssParser.generate(svgScope.scope.limit),
        converted.get(owner),
      );
      expect(stops).toEqual(limits);
    }
    expect(ids.size).toBe(2);
    const ownerScope = scopes.find(
      ({ scope }) =>
        scope.root && cssParser.generate(scope.root).startsWith(".owner"),
    )!;
    const ownerRoots = query(cssParser.generate(ownerScope.scope.root));
    expect(ownerRoots).toHaveLength(1);
    expect(ownerRoots[0]!.name).toBe("div");
  },
);
