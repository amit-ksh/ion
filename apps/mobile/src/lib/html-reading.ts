import type { BlockContent, PhrasingContent, RootContent } from "mdast";

const TEXT_LIMIT = 65_536;
const NODE_LIMIT = 1024;
const DEPTH_LIMIT = 32;
const ROW_LIMIT = 4096;
type Element = {
  tag: string;
  children: Content[];
  alt?: string;
  start?: number;
};
type Content = string | Element;
export type HtmlReading = { nodes: RootContent[]; omitted: boolean };

export function previewUtf8Size(text: string): number {
  let size = 0;
  for (const character of text) {
    const point = character.codePointAt(0)!;
    size += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
    if (size > TEXT_LIMIT) return size;
  }
  return size;
}
const entities: Record<string, string> = {
  AMP: "&",
  GT: ">",
  LT: "<",
  QUOT: '"',
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: "\u00a0",
  copy: "©",
  reg: "®",
  hellip: "…",
  ndash: "–",
  mdash: "—",
  middot: "·",
  laquo: "«",
  raquo: "»",
  bull: "•",
  trade: "™",
};
/** One pass only: decoded markup can never become parser input. */
function decode(text: string): string {
  return text.replace(
    /&(#x[0-9a-f]{1,8}|#[0-9]{1,10}|[a-z][a-z0-9]{0,31});/gi,
    (original: string, name: string) => {
      if (name[0] !== "#") return entities[name] ?? original;
      const point = /^#x/i.test(name)
        ? parseInt(name.slice(2), 16)
        : Number(name.slice(1));
      return point > 0 &&
        point <= 0x10ffff &&
        !(point >= 0xd800 && point <= 0xdfff)
        ? String.fromCodePoint(point)
        : "�";
    },
  );
}
const voidTags = new Set([
  "area",
  "frame",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);
const omittedTags = new Set([
  "script",
  "style",
  "head",
  "title",
  "template",
  "noscript",
]);
const mediaTags: Record<string, string> = {
  img: "Image",
  picture: "Image",
  video: "Video",
  audio: "Audio",
  iframe: "Frame",
  frame: "Frame",
  frameset: "Frames",
  object: "Embedded object",
  embed: "Embedded object",
  canvas: "Canvas",
  svg: "Vector image",
  math: "Math",
  form: "Form",
  button: "Form control",
  input: "Form control",
  select: "Form control",
  textarea: "Form control",
};
const rawTags = new Set(["script", "style", "title", "textarea"]);
const inlineTags = new Set([
  "a",
  "span",
  "strong",
  "b",
  "em",
  "i",
  "s",
  "del",
  "strike",
  "code",
  "kbd",
  "samp",
  "small",
  "sub",
  "sup",
  "mark",
  "u",
  "abbr",
  "time",
  "label",
  "q",
  "cite",
  "bdi",
  "bdo",
  "br",
  "wbr",
]);

/** Strict semantic subset, never a browser parser or CSS engine. Attributes are
 * discarded except bounded image alt text and an ordered-list starting number.
 * Ambiguous/malformed structures fail closed to complete original Source. */
function tag(source: string): {
  tag: string;
  close: boolean;
  selfClose: boolean;
  alt?: string;
  start?: number;
} {
  if (source.length > 4096) throw new Error("tag limit");
  const closing = /^\/\s*([a-z][a-z0-9:-]*)\s*$/i.exec(source);
  if (closing)
    return { tag: closing[1]!.toLowerCase(), close: true, selfClose: false };
  const opening = /^([a-z][a-z0-9:-]*)([\s\S]*)$/i.exec(source);
  if (!opening) throw new Error("malformed tag");
  const name = opening[1]!.toLowerCase();
  let rest = opening[2]!,
    selfClose = false,
    alt: string | undefined,
    start: number | undefined;
  if (rest.endsWith("/")) {
    selfClose = true;
    rest = rest.slice(0, -1);
  }
  if (rest && !/^\s/.test(rest)) throw new Error("malformed attributes");
  let offset = 0,
    attributes = 0;
  const seen = new Set<string>();
  while (offset < rest.length) {
    while (/\s/.test(rest[offset] ?? "") && offset < rest.length) offset++;
    if (offset === rest.length) break;
    const match = /^[a-z_:][a-z0-9_.:-]*/i.exec(rest.slice(offset));
    if (!match || ++attributes > 32) throw new Error("attribute limit");
    const key = match[0].toLowerCase();
    if (seen.has(key)) throw new Error("ambiguous attribute");
    seen.add(key);
    offset += match[0].length;
    while (/\s/.test(rest[offset] ?? "") && offset < rest.length) offset++;
    let value = "";
    if (rest[offset] === "=") {
      offset++;
      while (/\s/.test(rest[offset] ?? "") && offset < rest.length) offset++;
      const quote = rest[offset];
      if (quote === '"' || quote === "'") {
        const end = rest.indexOf(quote, offset + 1);
        if (end < 0) throw new Error("unclosed attribute");
        value = rest.slice(offset + 1, end);
        offset = end + 1;
      } else {
        const end = /^[^\s'"=<>`]+/.exec(rest.slice(offset));
        if (!end) throw new Error("malformed attribute");
        value = end[0];
        offset += value.length;
      }
      if (offset < rest.length && !/\s/.test(rest[offset]!))
        throw new Error("malformed attributes");
    }
    if (name === "img" && key === "alt") alt = decode(value).slice(0, 128);
    if (name === "ol" && key === "start" && /^[0-9]{1,6}$/.test(value))
      start = Number(value);
  }
  return { tag: name, close: false, selfClose, alt, start };
}
function nextTagEnd(text: string, from: number): number {
  let quote = "";
  for (let i = from; i < text.length; i++) {
    const char = text[i]!;
    if (i - from > 4096) throw new Error("tag limit");
    if (quote) {
      if (char === quote) quote = "";
    } else if (char === '"' || char === "'") quote = char;
    else if (char === ">") return i;
    else if (char === "<") throw new Error("malformed tag");
  }
  throw new Error("unclosed tag");
}
function flattened(content: Content[], literal = false): string {
  return content
    .map((item) =>
      typeof item === "string"
        ? item
        : flattened(item.children, literal) +
          (item.tag === "br" ? "\n" : literal ? "" : " "),
    )
    .join("");
}
function normalized(text: string): string {
  return text.replace(/[\t\r\n ]+/g, " ");
}
function plain(value: string): PhrasingContent {
  return { type: "text", value };
}
function trim(nodes: PhrasingContent[]): PhrasingContent[] {
  // Keep whitespace inside styled runs; only trim the document-flow boundaries.
  const first = nodes[0],
    last = nodes.at(-1);
  if (first?.type === "text") first.value = first.value.trimStart();
  if (last?.type === "text") last.value = last.value.trimEnd();
  return nodes.filter((node) => node.type !== "text" || node.value !== "");
}
function inlines(content: Content[]): PhrasingContent[] {
  return content.flatMap((item): PhrasingContent[] => {
    if (typeof item === "string") return [plain(normalized(item))];
    if (item.tag === "br") return [{ type: "break" }];
    if (item.tag === "code" || item.tag === "kbd" || item.tag === "samp")
      return [{ type: "inlineCode", value: flattened(item.children, true) }];
    const children = inlines(item.children);
    if (item.tag === "b" || item.tag === "strong")
      return [{ type: "strong", children }];
    if (item.tag === "i" || item.tag === "em" || item.tag === "cite")
      return [{ type: "emphasis", children }];
    if (["s", "del", "strike"].includes(item.tag))
      return [{ type: "delete", children }];
    return children;
  });
}
function blocks(content: Content[]): RootContent[] {
  const result: RootContent[] = [];
  let pending: Content[] = [];
  const flush = () => {
    const children = trim(inlines(pending));
    pending = [];
    if (children.length) result.push({ type: "paragraph", children });
  };
  for (const item of content) {
    if (typeof item === "string" || inlineTags.has(item.tag)) {
      pending.push(item);
      continue;
    }
    flush();
    const heading = /^h([1-6])$/.exec(item.tag);
    if (heading)
      result.push({
        type: "heading",
        depth: Number(heading[1]) as 1 | 2 | 3 | 4 | 5 | 6,
        children: trim(inlines(item.children)),
      });
    else if (
      item.tag === "p" ||
      item.tag === "dt" ||
      item.tag === "dd" ||
      item.tag === "figcaption"
    )
      result.push({
        type: "paragraph",
        children: trim(inlines(item.children)),
      });
    else if (item.tag === "pre")
      result.push({ type: "code", value: flattened(item.children, true) });
    else if (item.tag === "hr") result.push({ type: "thematicBreak" });
    else if (item.tag === "blockquote")
      result.push({
        type: "blockquote",
        children: blocks(item.children) as BlockContent[],
      });
    else if (item.tag === "ul" || item.tag === "ol") {
      let index = item.start ?? 1;
      for (const child of item.children) {
        if (typeof child === "string") {
          if (child.trim()) throw new Error("malformed list");
          continue;
        }
        if (child.tag !== "li") throw new Error("malformed list");
        // One item per top-level row keeps long lists virtualized.
        result.push({
          type: "list",
          ordered: item.tag === "ol",
          start: index++,
          spread: false,
          children: [
            {
              type: "listItem",
              spread: false,
              children: blocks(child.children) as BlockContent[],
            },
          ],
        });
      }
    } else if (item.tag === "table") {
      const rows: Extract<RootContent, { type: "table" }>["children"] = [];
      const visit = (children: Content[]) => {
        for (const row of children) {
          if (typeof row === "string") {
            if (row.trim()) throw new Error("malformed table");
            continue;
          }
          if (["thead", "tbody", "tfoot"].includes(row.tag)) {
            visit(row.children);
            continue;
          }
          if (row.tag === "caption") {
            result.push(...blocks(row.children));
            continue;
          }
          if (row.tag === "colgroup" || row.tag === "col") continue;
          if (row.tag !== "tr") throw new Error("malformed table");
          const cells: Extract<
            RootContent,
            { type: "table" }
          >["children"][number]["children"] = [];
          for (const cell of row.children) {
            if (typeof cell === "string") {
              if (cell.trim()) throw new Error("malformed table row");
              continue;
            }
            if (!["th", "td"].includes(cell.tag) || cells.length >= 16)
              throw new Error("table column limit");
            cells.push({
              type: "tableCell",
              children: trim(inlines(cell.children)),
            });
          }
          rows.push({ type: "tableRow", children: cells });
        }
      };
      visit(item.children);
      const head = rows[0];
      if (head) {
        if (rows.length === 1)
          result.push({ type: "table", align: [], children: [head] });
        for (let i = 1; i < rows.length; i += 8)
          result.push({
            type: "table",
            align: [],
            children: [head, ...rows.slice(i, i + 8)],
          });
      }
    } else result.push(...blocks(item.children));
  }
  flush();
  return result;
}
export function htmlNodeText(
  node: RootContent | Record<string, unknown>,
): string {
  if ("value" in node && typeof node.value === "string") return node.value;
  if (node.type === "break") return "\n";
  if ("children" in node && Array.isArray(node.children))
    return node.children
      .map((child) => htmlNodeText(child as Record<string, unknown>))
      .join(
        ["list", "listItem", "blockquote", "table"].includes(
          node.type as string,
        )
          ? "\n"
          : node.type === "tableRow"
            ? "\t"
            : "",
      );
  return "";
}
function bounded(nodes: RootContent[]): RootContent[] {
  let count = 0,
    bytes = 0;
  function check(node: Record<string, unknown>, depth: number) {
    if (++count > 4096 || depth > DEPTH_LIMIT)
      throw new Error("reading node limit");
    if (typeof node.value === "string") bytes += previewUtf8Size(node.value);
    if (bytes > TEXT_LIMIT) throw new Error("reading text limit");
    if (Array.isArray(node.children))
      for (const child of node.children)
        check(child as Record<string, unknown>, depth + 1);
  }
  for (const node of nodes)
    check(node as unknown as Record<string, unknown>, 0);
  return nodes.flatMap((node) => {
    const text = htmlNodeText(node);
    let rowNodes = 0;
    const countRow = (value: Record<string, unknown>) => {
      rowNodes++;
      if (Array.isArray(value.children))
        for (const child of value.children)
          countRow(child as Record<string, unknown>);
    };
    countRow(node as unknown as Record<string, unknown>);
    if (text.length <= ROW_LIMIT && rowNodes <= 128) return [node];
    // Preserve all visible text in bounded native paragraphs for giant blocks.
    return (text.match(/[\s\S]{1,1000}/gu) ?? []).map((value): RootContent => ({
      type: "paragraph",
      children: [plain(value)],
    }));
  });
}
/** Yield between bounded parser batches so cancellation/foreground fences get
 * checked before committing any derived view. No partially parsed view escapes. */
export function* htmlReadingSteps(text: string): Generator<void, HtmlReading> {
  if (previewUtf8Size(text) > TEXT_LIMIT) throw new Error("HTML text limit");
  const root: Element = { tag: "root", children: [] },
    stack = [root];
  let at = 0,
    nodes = 0,
    checkpoint = 0,
    nodeCheckpoint = 0,
    tokens = 0,
    omitted = false;
  const push = (content: Content) => {
    if (++nodes > NODE_LIMIT) throw new Error("HTML node limit");
    stack.at(-1)!.children.push(content);
  };
  while (at < text.length) {
    if (++tokens > 4096) throw new Error("HTML token limit");
    if (at - checkpoint >= 4096 || nodes - nodeCheckpoint >= 64) {
      checkpoint = at;
      nodeCheckpoint = nodes;
      yield;
    }
    if (text.startsWith("<!--", at)) {
      const end = text.indexOf("-->", at + 4);
      if (end < 0) throw new Error("unclosed comment");
      if (++nodes > NODE_LIMIT) throw new Error("HTML node limit");
      at = end + 3;
      continue;
    }
    if (text[at] !== "<" || !/[a-z/!?]/i.test(text[at + 1] ?? "")) {
      let end = text.indexOf("<", at + 1);
      if (end < 0) end = text.length;
      push(decode(text.slice(at, end)));
      at = end;
      continue;
    }
    const end = nextTagEnd(text, at + 1),
      source = text.slice(at + 1, end);
    if (/^!doctype\s+html\s*$/i.test(source)) {
      at = end + 1;
      continue;
    }
    const parsed = tag(source);
    at = end + 1;
    if (parsed.close) {
      if (stack.length === 1 || stack.at(-1)!.tag !== parsed.tag)
        throw new Error("unmatched closing tag");
      stack.pop();
      continue;
    }
    if (rawTags.has(parsed.tag)) {
      if (parsed.selfClose) throw new Error("malformed raw tag");
      const close = new RegExp(`</${parsed.tag}\\s*>`, "gi");
      close.lastIndex = at;
      const match = close.exec(text);
      if (!match) throw new Error("unclosed raw tag");
      omitted = true;
      if (mediaTags[parsed.tag])
        push(`${mediaTags[parsed.tag]} not displayed.`);
      else if (++nodes > NODE_LIMIT) throw new Error("HTML node limit");
      at = match.index + match[0].length;
      continue;
    }
    const element: Element = {
      tag: parsed.tag,
      children: [],
      alt: parsed.alt,
      start: parsed.start,
    };
    push(element);
    if (!voidTags.has(parsed.tag) && !parsed.selfClose) {
      if (stack.length >= DEPTH_LIMIT) throw new Error("HTML nesting limit");
      stack.push(element);
    }
  }
  if (stack.length !== 1) throw new Error("unclosed element");
  yield;
  function remove(content: Content[]): Content[] {
    return content.flatMap((item): Content[] => {
      if (typeof item === "string") return [item];
      if (omittedTags.has(item.tag)) {
        omitted = true;
        return [];
      }
      const kind = mediaTags[item.tag];
      if (kind) {
        omitted = true;
        return [
          {
            tag: "p",
            children: [
              `${kind} not displayed${item.alt ? `: ${item.alt}` : "."}`,
            ],
          },
        ];
      }
      return [{ ...item, children: remove(item.children) }];
    });
  }
  const output = bounded(blocks(remove(root.children)));
  return {
    nodes: output.length
      ? output
      : [
          {
            type: "paragraph",
            children: [
              plain(
                "No readable HTML content. Use Source or Export to inspect the original.",
              ),
            ],
          },
        ],
    omitted,
  };
}
export function readHtml(text: string): HtmlReading {
  const steps = htmlReadingSteps(text);
  let result = steps.next();
  while (!result.done) result = steps.next();
  return result.value;
}
