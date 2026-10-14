import type {
  DocumentPreviewRow,
  DocumentReadingPreview,
} from "./document-preview";
import {
  officeAllowed,
  officeAttribute as attr,
  officePart,
  openOfficePackage,
  readOfficeXml,
  type OfficeScope,
  type OfficeTag,
} from "./office-package";
const PACKAGE = "http://schemas.openxmlformats.org/package/2006/relationships";
const TYPES = "http://schemas.openxmlformats.org/package/2006/content-types";
const REL = [
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
  "http://purl.oclc.org/ooxml/officeDocument/relationships",
];
const WORD = [
  "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
  "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const SHEET = [
  "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
  "http://purl.oclc.org/ooxml/spreadsheetml/main",
];
const MC = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const is = (tag: OfficeTag | undefined, namespaces: string[], local: string) =>
  !!tag && namespaces.includes(tag.uri) && tag.local === local;
const word = (tag: OfficeTag, local: string) => is(tag, WORD, local);
const sheet = (tag: OfficeTag | undefined, local: string) =>
  is(tag, SHEET, local);
const relationshipPath = (part: string) => {
  const path = part.split("/"),
    file = path.pop()!;
  return [...path, "_rels", `${file}.rels`].join("/");
};
type Package = Awaited<ReturnType<typeof openOfficePackage>>;
type Relationship = { type: string; target: string; external: boolean };
async function relationships(pkg: Package, base: string, current: OfficeScope) {
  const result = new Map<string, Relationship>();
  const path = base ? relationshipPath(base) : "_rels/.rels";
  await readOfficeXml(await pkg.read(path), current, {
    open(tag, ancestors) {
      if (
        !ancestors.length &&
        (tag.uri !== PACKAGE || tag.local !== "Relationships")
      )
        throw new Error("Invalid Office relationships root.");
      if (
        ancestors.length !== 1 ||
        tag.uri !== PACKAGE ||
        tag.local !== "Relationship"
      )
        return;
      const id = attr(tag, "Id"),
        type = attr(tag, "Type"),
        target = attr(tag, "Target"),
        mode = attr(tag, "TargetMode");
      if (
        !id ||
        !type ||
        !target ||
        result.has(id) ||
        (mode && !["External", "Internal"].includes(mode))
      )
        throw new Error("Invalid or duplicate Office relationship.");
      result.set(id, { type, target, external: mode === "External" });
    },
  });
  return result;
}
function requiredRelation(
  rel: Relationship | undefined,
  kind: string,
  base: string,
) {
  if (!rel || rel.external || !REL.some((uri) => rel.type === `${uri}/${kind}`))
    throw new Error(`Office ${kind} relationship is missing or external.`);
  return officePart(base, rel.target);
}
async function mainPart(
  pkg: Package,
  format: "docx" | "xlsx",
  current: OfficeScope,
) {
  const rels = await relationships(pkg, "", current);
  const mains = [...rels.values()].filter((rel) =>
    REL.some((uri) => rel.type === `${uri}/officeDocument`),
  );
  if (mains.length !== 1)
    throw new Error("Office package must identify exactly one main document.");
  const part = requiredRelation(mains[0], "officeDocument", "");
  const expected =
    format === "docx"
      ? "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
      : "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
  const overrides = new Map<string, string>();
  await readOfficeXml(await pkg.read("[Content_Types].xml"), current, {
    open(tag, ancestors) {
      if (!ancestors.length && (tag.uri !== TYPES || tag.local !== "Types"))
        throw new Error("Invalid Office content-types root.");
      if (
        ancestors.length !== 1 ||
        tag.uri !== TYPES ||
        tag.local !== "Override"
      )
        return;
      const name = attr(tag, "PartName"),
        type = attr(tag, "ContentType");
      if (!name?.startsWith("/") || !type)
        throw new Error("Invalid Office content type.");
      const resolved = officePart("", name);
      if (overrides.has(resolved))
        throw new Error("Duplicate Office content type.");
      overrides.set(resolved, type);
    },
  });
  if (overrides.get(part) !== expected)
    throw new Error("Office format does not match the published document.");
  return part;
}
class Reading {
  rows: DocumentPreviewRow[] = [];
  size = 0;
  add(text: string, heading = 0) {
    if (!text) return;
    this.size += text.length;
    if (this.size > 512 * 1024)
      throw new Error(
        "Extracted Office text exceeds reading limits. Export the original.",
      );
    for (const chunk of text.match(/[\s\S]{1,1000}/gu) ?? []) {
      if (this.rows.length >= 12_000)
        throw new Error(
          "Office has too many reading rows. Export the original.",
        );
      this.rows.push({
        id: String(this.rows.length),
        text: chunk,
        node: heading
          ? {
              type: "heading",
              depth: Math.min(6, Math.max(1, heading)) as 1,
              children: [{ type: "text", value: chunk }],
            }
          : { type: "paragraph", children: [{ type: "text", value: chunk }] },
      });
    }
  }
}
async function readWord(
  pkg: Package,
  part: string,
  current: OfficeScope,
): Promise<DocumentReadingPreview> {
  const reading = new Reading(),
    tables: { id: number; row: number; cell: number }[] = [];
  let paragraph: { text: string; heading: number } | null = null,
    tableCount = 0;
  const ignored = (ancestors: OfficeTag[]) =>
    ancestors.some(
      (tag) =>
        (WORD.includes(tag.uri) &&
          ["del", "moveFrom", "instrText"].includes(tag.local)) ||
        (tag.uri === MC && tag.local === "Choice"),
    );
  await readOfficeXml(await pkg.read(part), current, {
    open(tag, ancestors) {
      if (!ancestors.length && !word(tag, "document"))
        throw new Error("Invalid Word document root.");
      if (
        ignored([...ancestors, tag]) ||
        !ancestors.some((tag) => word(tag, "body"))
      )
        return;
      if (word(tag, "tbl")) tables.push({ id: ++tableCount, row: 0, cell: 0 });
      if (word(tag, "tr") && tables.length) {
        tables.at(-1)!.row++;
        tables.at(-1)!.cell = 0;
      }
      if (word(tag, "tc") && tables.length) tables.at(-1)!.cell++;
      if (word(tag, "p")) {
        if (paragraph) throw new Error("Unsupported nested Word paragraph.");
        paragraph = { text: "", heading: 0 };
      }
      if (!paragraph) return;
      if (word(tag, "pStyle")) {
        const value =
          WORD.map((uri) => attr(tag, "val", uri)).find(Boolean) ?? "";
        paragraph.heading = Number(/^Heading([1-6])$/i.exec(value)?.[1] ?? 0);
      }
      if (word(tag, "tab")) paragraph.text += "\t";
      if (word(tag, "br") || word(tag, "cr")) paragraph.text += "\n";
      if (
        tag.local === "docPr" &&
        tag.uri ===
          "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
      ) {
        const label = attr(tag, "descr") ?? attr(tag, "title");
        if (label) paragraph.text += `[Image: ${label}]`;
      }
    },
    text(text, ancestors) {
      if (paragraph && word(ancestors.at(-1)!, "t") && !ignored(ancestors))
        paragraph.text += text;
    },
    close(tag, ancestors) {
      if (ignored([...ancestors, tag])) return;
      if (word(tag, "p") && paragraph) {
        const table = tables.at(-1);
        reading.add(
          table
            ? `Table ${table.id} · Row ${table.row} · Cell ${table.cell}\n${paragraph.text}`
            : paragraph.text,
          paragraph.heading,
        );
        paragraph = null;
      }
      if (word(tag, "tbl")) tables.pop();
    },
  });
  return {
    rows: reading.rows,
    sourceFallback: false,
    note: "Extracted body text and table cells. Page layout, styles, numbering, media, headers, footers, notes and comments are not reproduced. Deleted text and field instructions are omitted; inserted text is included. Export the original for the complete document.",
  };
}

async function sharedStrings(pkg: Package, part: string, current: OfficeScope) {
  const strings: string[] = [];
  let value: string | null = null,
    total = 0;
  await readOfficeXml(await pkg.read(part), current, {
    open(tag, ancestors) {
      if (!ancestors.length && !sheet(tag, "sst"))
        throw new Error("Invalid shared strings root.");
      if (sheet(tag, "si") && ancestors.length === 1) value = "";
    },
    text(text, ancestors) {
      if (
        value !== null &&
        sheet(ancestors.at(-1), "t") &&
        !ancestors.some((tag) => sheet(tag, "rPh"))
      )
        value += text;
    },
    close(tag, ancestors) {
      if (sheet(tag, "si") && ancestors.length === 1 && value !== null) {
        total += value.length;
        if (strings.length >= 50_000 || total > 2 * 1024 * 1024)
          throw new Error("Shared strings exceed Office reading limits.");
        strings.push(value);
        value = null;
      }
    },
  });
  return strings;
}
function cellReference(value: string) {
  const match = /^([A-Z]{1,3})([1-9][0-9]{0,6})$/.exec(value);
  if (!match) throw new Error("Invalid worksheet cell reference.");
  let column = 0;
  for (const letter of match[1]!)
    column = column * 26 + letter.charCodeAt(0) - 64;
  const row = Number(match[2]);
  if (column > 16384 || row > 1048576)
    throw new Error("Worksheet cell exceeds Excel limits.");
  return { column, row };
}
async function readWorkbook(
  pkg: Package,
  part: string,
  current: OfficeScope,
): Promise<DocumentReadingPreview> {
  const rels = await relationships(pkg, part, current),
    sheets: { name: string; state: string; id: string }[] = [];
  await readOfficeXml(await pkg.read(part), current, {
    open(tag, ancestors) {
      if (!ancestors.length && !sheet(tag, "workbook"))
        throw new Error("Invalid workbook root.");
      if (!sheet(tag, "sheet") || !sheet(ancestors.at(-1), "sheets")) return;
      const name = attr(tag, "name"),
        id = REL.map((uri) => attr(tag, "id", uri)).find(Boolean),
        state = attr(tag, "state") ?? "visible";
      if (
        !name ||
        name.length > 128 ||
        !id ||
        sheets.some((sheet) => sheet.name === name || sheet.id === id) ||
        !["visible", "hidden", "veryHidden"].includes(state) ||
        sheets.length >= 32
      )
        throw new Error("Unsupported or duplicate workbook sheet.");
      sheets.push({ name, state, id });
    },
  });
  if (!sheets.length) throw new Error("Workbook has no readable sheets.");
  const shared = [...rels.values()].filter((rel) =>
    REL.some((uri) => rel.type === `${uri}/sharedStrings`),
  );
  if (shared.length > 1)
    throw new Error("Duplicate shared-strings relationship.");
  const strings = shared.length
    ? await sharedStrings(
        pkg,
        requiredRelation(shared[0], "sharedStrings", part),
        current,
      )
    : [];
  const reading = new Reading();
  let cells = 0,
    rowCount = 0;
  const sheetParts = new Set<string>();
  for (const record of sheets) {
    reading.add(
      `${record.name}${record.state === "visible" ? "" : ` (${record.state})`}`,
      2,
    );
    const path = requiredRelation(rels.get(record.id), "worksheet", part);
    if (sheetParts.has(path))
      throw new Error("Workbook sheets share the same part.");
    sheetParts.add(path);
    let row = 0,
      previousRow = 0,
      column = 0,
      rowExplicit = false,
      cell: {
        reference: string;
        type: string;
        value: string;
        formula: string;
        hasFormula: boolean;
        hasValue: boolean;
        inline: string;
      } | null = null;
    await readOfficeXml(await pkg.read(path), current, {
      open(tag, ancestors) {
        if (!ancestors.length && !sheet(tag, "worksheet"))
          throw new Error("Invalid worksheet root.");
        if (sheet(tag, "row") && sheet(ancestors.at(-1), "sheetData")) {
          const value = attr(tag, "r");
          rowExplicit = value !== undefined;
          row = value === undefined ? previousRow + 1 : Number(value);
          if (
            !Number.isInteger(row) ||
            row <= previousRow ||
            row > 1048576 ||
            ++rowCount > 10_000
          )
            throw new Error(
              "Worksheet rows exceed reading limits or are unordered.",
            );
          column = 0;
        }
        if (sheet(tag, "c") && sheet(ancestors.at(-1), "row") && row) {
          if (cell || ++cells > 20_000)
            throw new Error("Worksheet cells exceed reading limits.");
          const reference = attr(tag, "r");
          // Missing references are legal OOXML: infer the next column, preserving gaps when explicit.
          let label = reference;
          if (!label) {
            let n = column + 1,
              letters = "";
            while (n) {
              letters = String.fromCharCode(65 + ((n - 1) % 26)) + letters;
              n = Math.floor((n - 1) / 26);
            }
            label = `${letters}${row}`;
          }
          const parsed = cellReference(label);
          if (!rowExplicit && column === 0) {
            if (parsed.row <= previousRow)
              throw new Error("Worksheet rows are unordered.");
            row = parsed.row;
          }
          if (parsed.row !== row || parsed.column <= column)
            throw new Error("Worksheet cells are duplicated or unordered.");
          column = parsed.column;
          cell = {
            reference: label,
            type: attr(tag, "t") ?? "n",
            value: "",
            formula: "",
            hasFormula: false,
            hasValue: false,
            inline: "",
          };
        }
        if (cell && sheet(tag, "v") && sheet(ancestors.at(-1), "c"))
          cell.hasValue = true;
        if (cell && sheet(tag, "f") && sheet(ancestors.at(-1), "c"))
          cell.hasFormula = true;
      },
      text(text, ancestors) {
        if (!cell) return;
        const parent = ancestors.at(-1);
        if (sheet(parent, "v") && sheet(ancestors.at(-2), "c"))
          cell.value += text;
        if (sheet(parent, "f") && sheet(ancestors.at(-2), "c"))
          cell.formula += text;
        if (
          sheet(parent, "t") &&
          ancestors.some((tag) => sheet(tag, "is")) &&
          !ancestors.some((tag) => sheet(tag, "rPh"))
        )
          cell.inline += text;
      },
      close(tag) {
        if (sheet(tag, "row") && row) {
          previousRow = row;
          row = 0;
        }
        if (!sheet(tag, "c") || !cell) return;
        let value = cell.value;
        if (cell.hasFormula && !cell.hasValue) value = "Unavailable";
        else if (cell.type === "s") {
          if (
            !/^(?:0|[1-9][0-9]*)$/.test(value) ||
            strings[Number(value)] === undefined
          )
            throw new Error("Worksheet shared string is missing.");
          value = strings[Number(value)]!;
        } else if (cell.type === "inlineStr") value = cell.inline;
        else if (cell.type === "b") {
          if (!["0", "1"].includes(value))
            throw new Error("Invalid worksheet boolean.");
          value = value === "1" ? "TRUE" : "FALSE";
        } else if (!["n", "str", "e", "d"].includes(cell.type))
          throw new Error("Unsupported worksheet cell type.");
        if (cell.hasFormula)
          value = `${cell.formula ? `=${cell.formula}` : "Formula expression not stored in this cell"}\nSaved value: ${value || (cell.hasValue ? "(empty)" : "Unavailable")}`;
        if (value) reading.add(`${cell.reference}: ${value}`);
        cell = null;
      },
    });
  }
  return {
    rows: reading.rows,
    sourceFallback: false,
    note: "Extracted cells in sheet order, including hidden sheets. Formulas are not run; saved values may be stale. Numbers and dates use stored values without Excel formatting. Charts, images, comments and layout are not reproduced. Export the original for the complete workbook.",
  };
}

/** Call only after the original's immutable metadata and SHA-256 have been verified. */
export async function prepareOfficeReading(
  format: string,
  bytes: Uint8Array,
  current: OfficeScope,
): Promise<DocumentReadingPreview> {
  if (format !== "docx" && format !== "xlsx")
    throw new Error("Unsupported Office reading format.");
  const pkg = await openOfficePackage(bytes, current),
    part = await mainPart(pkg, format, current);
  const result =
    format === "docx"
      ? await readWord(pkg, part, current)
      : await readWorkbook(pkg, part, current);
  officeAllowed(current);
  return result;
}
