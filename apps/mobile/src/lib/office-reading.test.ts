import { expect, test } from "bun:test";
import { Zip, ZipDeflate, strToU8, zipSync } from "fflate/browser";
import { openOfficePackage, officePart, readOfficeXml } from "./office-package";
import { prepareOfficeReading } from "./office-reading";
const R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const S = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const P = "http://schemas.openxmlformats.org/package/2006/relationships";
const C = "http://schemas.openxmlformats.org/package/2006/content-types";
const types = (format: string, main: string) =>
  `<Types xmlns="${C}"><Override PartName="/${main}" ContentType="application/vnd.openxmlformats-officedocument.${format === "docx" ? "wordprocessingml.document" : "spreadsheetml.sheet"}.main+xml"/></Types>`;
function archive(files: Record<string, string | Uint8Array>, stored = false) {
  return zipSync(
    Object.fromEntries(
      Object.entries(files).map(([name, text]) => [
        name,
        typeof text === "string" ? strToU8(text) : text,
      ]),
    ),
    { level: stored ? 0 : 6 },
  );
}
function parts(format: "docx" | "xlsx", text: string, target?: string) {
  const main =
    target ?? (format === "docx" ? "word/document.xml" : "xl/workbook.xml");
  return {
    "[Content_Types].xml": types(format, main),
    "_rels/.rels": `<Relationships xmlns="${P}"><Relationship Id="main" Type="${R}/officeDocument" Target="${main}"/></Relationships>`,
    [main]: text,
  };
}
const word = (body: string) =>
  `<x:document xmlns:x="${W}"><x:body>${body}</x:body></x:document>`;
const plain = (text: string) =>
  word(`<x:p><x:r><x:t>${text}</x:t></x:r></x:p>`);
const visible = (view: Awaited<ReturnType<typeof prepareOfficeReading>>) =>
  view.rows.map((row) => row.text).join("\n");
function worksheet(body: string) {
  return `<worksheet xmlns="${S}"><sheetData>${body}</sheetData></worksheet>`;
}
function workbook(body: string, firstSheet: string, secondSheet = "") {
  return {
    ...parts(
      "xlsx",
      `<workbook xmlns="${S}" xmlns:r="${R}"><sheets>${body}</sheets></workbook>`,
    ),
    "xl/_rels/workbook.xml.rels": `<Relationships xmlns="${P}"><Relationship Id="s1" Type="${R}/worksheet" Target="worksheets/one.xml"/><Relationship Id="s2" Type="${R}/worksheet" Target="worksheets/two.xml"/><Relationship Id="strings" Type="${R}/sharedStrings" Target="sharedStrings.xml"/></Relationships>`,
    "xl/sharedStrings.xml": `<sst xmlns="${S}"><si><r><t>Client </t></r><r><t>&amp; research</t></r><rPh><t>phonetic-only</t></rPh></si></sst>`,
    "xl/worksheets/one.xml": worksheet(firstSheet),
    "xl/worksheets/two.xml": worksheet(secondSheet),
  };
}
function directory(bytes: Uint8Array) {
  const b = Buffer.from(bytes),
    start = b.readUInt32LE(b.length - 6),
    entries: number[] = [];
  let p = start;
  while (b.readUInt32LE(p) === 0x02014b50) {
    entries.push(p);
    p +=
      46 +
      b.readUInt16LE(p + 28) +
      b.readUInt16LE(p + 30) +
      b.readUInt16LE(p + 32);
  }
  return { b, entries };
}

test("DOCX body preserves order, Unicode, literal markup, headings and table locations", async () => {
  const body = `<x:p><x:pPr><x:pStyle x:val="Heading1"/></x:pPr><x:r><x:t>Plan 🚀</x:t></x:r></x:p><x:p><x:r><x:t>&lt;script&gt;literal&lt;/script&gt; &amp; safe</x:t><x:tab/><x:t>next</x:t><x:br/><x:t>line</x:t></x:r><x:del><x:r><x:delText>deleted</x:delText><x:t>also deleted</x:t></x:r></x:del><x:r><x:instrText>EXEC malicious</x:instrText></x:r><x:ins><x:r><x:t> inserted</x:t></x:r></x:ins></x:p><x:tbl><x:tr><x:tc><x:p><x:r><x:t>Budget</x:t></x:r></x:p></x:tc><x:tc><x:p><x:r><x:t>12</x:t></x:r></x:p></x:tc></x:tr></x:tbl>`;
  const bytes = archive(parts("docx", word(body))),
    before = bytes.slice();
  const view = await prepareOfficeReading("docx", bytes, () => true),
    text = visible(view);
  expect(view.rows[0]?.node?.type).toBe("heading");
  expect(text).toContain(
    "Plan 🚀\n<script>literal</script> & safe\tnext\nline inserted",
  );
  expect(text).toContain("Table 1 · Row 1 · Cell 2\n12");
  expect(text).not.toContain("deleted");
  expect(text).not.toContain("EXEC");
  expect(view.note).toContain("not reproduced");
  expect(bytes).toEqual(before);
});

test("XLSX follows sheet order and shows hidden sheets, strings, sparse references and saved formulas", async () => {
  const files = workbook(
    `<sheet name="Archive" state="hidden" sheetId="2" r:id="s2"/><sheet name="Current" sheetId="1" r:id="s1"/>`,
    `<row r="3"><c r="A3" t="s"><v>0</v></c><c r="C3" t="inlineStr"><is><r><t>Hi 🚀</t></r></is></c><c r="D3"><f>SUM(A1:A2)</f><v>42</v></c><c r="E3" t="b"><v>1</v></c><c r="F3" t="e"><v>#DIV/0!</v></c><c r="G3" t="d"><v>2026-10-14</v></c></row>`,
    `<row><c><v>1</v></c><c><v>2</v></c></row>`,
  );
  const text = visible(
    await prepareOfficeReading("xlsx", archive(files), () => true),
  );
  expect(text.indexOf("Archive (hidden)")).toBeLessThan(
    text.indexOf("Current"),
  );
  for (const value of [
    "A1: 1",
    "B1: 2",
    "A3: Client & research",
    "C3: Hi 🚀",
    "D3: =SUM(A1:A2)\nSaved value: 42",
    "E3: TRUE",
    "F3: #DIV/0!",
    "G3: 2026-10-14",
  ])
    expect(text).toContain(value);
  expect(text).not.toContain("phonetic-only");
});

test("stored ZIP parts and streamed descriptor ZIP parts are both readable", async () => {
  const files = parts("docx", plain("streamed"));
  expect(
    visible(
      await prepareOfficeReading("docx", archive(files, true), () => true),
    ),
  ).toBe("streamed");
  const chunks: Uint8Array[] = [],
    zip = new Zip((error, data) => {
      if (error) throw error;
      chunks.push(data);
    });
  for (const [name, text] of Object.entries(files)) {
    const item = new ZipDeflate(name);
    zip.add(item);
    item.push(strToU8(text), true);
  }
  zip.end();
  const b = Buffer.concat(chunks);
  expect(visible(await prepareOfficeReading("docx", b, () => true))).toBe(
    "streamed",
  );
});

test("strict OOXML namespaces and percent-encoded spaces resolve without executing links", async () => {
  let files = parts("docx", plain("Strict"));
  files = Object.fromEntries(
    Object.entries(files).map(([name, value]) => [
      name,
      value
        .replaceAll(W, "http://purl.oclc.org/ooxml/wordprocessingml/main")
        .replaceAll(
          R,
          "http://purl.oclc.org/ooxml/officeDocument/relationships",
        ),
    ]),
  ) as typeof files;
  expect(
    visible(await prepareOfficeReading("docx", archive(files), () => true)),
  ).toBe("Strict");
  expect(officePart("word/document.xml", "../custom/My%20report.xml")).toBe(
    "custom/My report.xml",
  );
  const moved = parts("docx", plain("Moved"), "custom/report.xml");
  expect(
    visible(await prepareOfficeReading("docx", archive(moved), () => true)),
  ).toBe("Moved");
});

test("format spoofing, duplicate main relationships and external main parts refuse extraction", async () => {
  const files = parts("docx", plain("Never"));
  await expect(
    prepareOfficeReading("xlsx", archive(files), () => true),
  ).rejects.toThrow("format");
  for (const root of [
    files["_rels/.rels"].replace(
      'Target="word/',
      'TargetMode="External" Target="https://example.test/word/',
    ),
    files["_rels/.rels"].replace(
      "</Relationships>",
      `<Relationship Id="another" Type="${R}/officeDocument" Target="word/document.xml"/></Relationships>`,
    ),
    files["_rels/.rels"].replace(
      'Target="word/document.xml"',
      'Target="../outside.xml"',
    ),
  ])
    await expect(
      prepareOfficeReading(
        "docx",
        archive({ ...files, "_rels/.rels": root }),
        () => true,
      ),
    ).rejects.toThrow();
});

test("malformed XML, DTD/entities and excessive depth are refused", async () => {
  for (const xml of [
    plain("A &unknown;"),
    plain("bad").replace("</x:p>", "</x:q>"),
    `<!DOCTYPE x:document [<!ENTITY x SYSTEM "file:///private">]>${plain("&x;")}`,
    plain("body")
      .replace("<x:p>", "<x:p>" + "<x:a>".repeat(130))
      .replace("</x:p>", "</x:a>".repeat(130) + "</x:p>"),
  ])
    await expect(
      prepareOfficeReading("docx", archive(parts("docx", xml)), () => true),
    ).rejects.toThrow();
  await expect(
    readOfficeXml('<root xmlns="wrong" a="1" a="2"/>', () => true, {}),
  ).rejects.toThrow();
});

test("unsafe ZIP paths, duplicates, case collisions and links are refused", async () => {
  for (const name of [
    "../outside.xml",
    "/absolute.xml",
    "word\\document.xml",
    "word/%2e.xml",
    "word//bad.xml",
  ])
    await expect(
      openOfficePackage(archive({ [name]: "x" }), () => true),
    ).rejects.toThrow();
  const collision = archive({ "one.xml": "a", "ONE.xml": "b" });
  await expect(openOfficePackage(collision, () => true)).rejects.toThrow(
    "name",
  );
  const { b, entries } = directory(archive({ "link.xml": "x" }));
  b.writeUInt32LE((0xa000 << 16) >>> 0, entries[0]! + 38);
  await expect(openOfficePackage(b, () => true)).rejects.toThrow("name");
});

test("ZIP header disagreement, encryption, overlapping entries and corrupt CRC refuse reading", async () => {
  const original = archive(parts("docx", plain("Checksum")), true);
  const { b, entries } = directory(original),
    first = entries[0]!,
    local = b.readUInt32LE(first + 42);
  const flags = Buffer.from(b);
  flags.writeUInt16LE(1, first + 8);
  const method = Buffer.from(b);
  method.writeUInt16LE(99, first + 10);
  const names = Buffer.from(b);
  names[local + 30] ^= 1;
  const payload = Buffer.from(b),
    offset =
      local + 30 + b.readUInt16LE(local + 26) + b.readUInt16LE(local + 28);
  payload[offset] ^= 1;
  const overlap = Buffer.from(b);
  overlap.writeUInt32LE(local, entries[1]! + 42);
  for (const bytes of [
    flags,
    method,
    names,
    overlap,
    b.subarray(0, b.length - 1),
    Buffer.concat([b, Buffer.from([1])]),
  ])
    await expect(openOfficePackage(bytes, () => true)).rejects.toThrow();
  const pkg = await openOfficePackage(payload, () => true);
  await expect(pkg.read("[Content_Types].xml")).rejects.toThrow("checksum");
});

test("ZIP expansion is bounded by both declared size and actual decompressed bytes", async () => {
  const source = archive({
    "large.xml": "<r>" + "a".repeat(2 * 1024 * 1024) + "</r>",
  });
  const pkg = await openOfficePackage(source, () => true);
  await expect(pkg.read("large.xml")).rejects.toThrow("reading limits");
  const { b, entries } = directory(
    archive({ "bomb.xml": "a".repeat(1024 * 1024) }),
  );
  const entry = entries[0]!,
    local = b.readUInt32LE(entry + 42);
  b.writeUInt32LE(10, entry + 24);
  b.writeUInt32LE(10, local + 22);
  const bomb = await openOfficePackage(b, () => true);
  await expect(bomb.read("bomb.xml")).rejects.toThrow("declared size");
});

test("large DOCX reading yields and retains supplementary Unicode at row boundaries", async () => {
  const text = "🚀".repeat(80_000),
    file = archive(parts("docx", plain(text)));
  let ticks = 0;
  const timer = setInterval(() => ticks++, 1);
  try {
    const view = await prepareOfficeReading("docx", file, () => true);
    expect(view.rows.map((row) => row.text).join("")).toBe(text);
    expect(view.rows.every((row) => !/[\ud800-\udbff]$/.test(row.text))).toBe(
      true,
    );
    expect(ticks).toBeGreaterThan(8);
  } finally {
    clearInterval(timer);
  }
});

test("changed owner/grant scope cancels indexing, decompression and XML preparation", async () => {
  const bytes = archive(parts("docx", plain("A".repeat(160_000))));
  for (const limit of [1, 12, 70]) {
    let checks = 0;
    await expect(
      prepareOfficeReading("docx", bytes, () => ++checks < limit),
    ).rejects.toThrow("access changed");
  }
});

test("invalid spreadsheet references, shared strings, missing parts and external sheet targets refuse", async () => {
  for (const content of [
    '<row r="1"><c r="XFE1"><v>1</v></c></row>',
    '<row r="1"><c r="A2"><v>1</v></c></row>',
    '<row r="1"><c r="A1" t="s"><v>9</v></c></row>',
    '<row r="1"><c r="A1" t="b"><v>maybe</v></c></row>',
    '<row r="1"><c r="A1"><v>1</v></c><c r="A1"><v>2</v></c></row>',
  ])
    await expect(
      prepareOfficeReading(
        "xlsx",
        archive(workbook('<sheet name="Data" r:id="s1"/>', content)),
        () => true,
      ),
    ).rejects.toThrow();
  const files = workbook('<sheet name="Data" r:id="s1"/>', "");
  files["xl/_rels/workbook.xml.rels"] = files[
    "xl/_rels/workbook.xml.rels"
  ].replace(
    'Target="worksheets/one.xml"',
    'TargetMode="External" Target="https://example.test/one.xml"',
  );
  await expect(
    prepareOfficeReading("xlsx", archive(files), () => true),
  ).rejects.toThrow("external");
});

test("scope changes fence decompression and XML even after package indexing", async () => {
  let current = true;
  const bytes = archive({ "data.xml": "<r>" + "a".repeat(100_000) + "</r>" });
  const pkg = await openOfficePackage(bytes, () => current);
  setTimeout(() => {
    current = false;
  }, 0);
  await expect(pkg.read("data.xml")).rejects.toThrow("access changed");
  current = true;
  setTimeout(() => {
    current = false;
  }, 0);
  await expect(
    readOfficeXml("<r>" + "a".repeat(100_000) + "</r>", () => current, {}),
  ).rejects.toThrow("access changed");
});

test("shared/array formulas without a local formula or cached value are identified", async () => {
  const files = workbook(
    '<sheet name="Data" r:id="s1"/>',
    '<row r="1"><c r="A1"><f t="shared" si="0">1+1</f><v>2</v></c><c r="B1"><f t="shared" si="0"/><v>2</v></c><c r="C1"><f>NOW()</f></c></row>',
  );
  const view = await prepareOfficeReading("xlsx", archive(files), () => true);
  expect(visible(view)).toContain(
    "B1: Formula expression not stored in this cell\nSaved value: 2",
  );
  expect(visible(view)).toContain("C1: =NOW()\nSaved value: Unavailable");
});

test("omitted row indices use explicit cell references and cached empty strings stay distinct", async () => {
  const files = workbook(
    '<sheet name="Data" r:id="s1"/>',
    '<row><c r="D5"><v>5</v></c></row><row><c t="str"><f>IF(1, &quot;&quot;, &quot;x&quot;)</f><v/></c></row>',
  );
  const text = visible(
    await prepareOfficeReading("xlsx", archive(files), () => true),
  );
  expect(text).toContain("D5: 5");
  expect(text).toContain('A6: =IF(1, "", "x")\nSaved value: (empty)');
});
