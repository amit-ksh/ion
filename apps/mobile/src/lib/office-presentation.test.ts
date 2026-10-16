import { expect, test } from "bun:test";
import { createHash, randomUUID } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { strToU8, zipSync } from "fflate/browser";
import type {
  ArtifactLibraryItem,
  PortfolioQuery,
  PortfolioResult,
  WakuClient,
  WorkflowDocument,
} from "@waku/client";
import {
  artifactText,
  readArtifactDocument,
  validateArtifactItem,
} from "./artifact-library";
import {
  readGatewayWorkflowDocument,
  workflowDocumentMime,
  workflowExportName,
} from "./gateway-workflow-document";
import { prepareOfficeReading } from "./office-reading";
import type { GatewayPairing } from "./gateway-client";

const P = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const OPC = "http://schemas.openxmlformats.org/package/2006/relationships";
const T = "http://schemas.openxmlformats.org/package/2006/content-types";
const mainType =
  "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const slideType =
  "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const escape = (value: string) =>
  value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
const paragraph = (value: string) =>
  "<a:p><a:r><a:t>" + escape(value) + "</a:t></a:r></a:p>";
const shape = (value: string, title = false) =>
  '<p:sp><p:nvSpPr><p:cNvPr id="2" name="Text"/><p:cNvSpPr/><p:nvPr>' +
  (title ? '<p:ph type="title"/>' : "") +
  "</p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>" +
  paragraph(value) +
  "</p:txBody></p:sp>";
const slide = (body: string, show = "true") =>
  '<p:sld xmlns:p="' +
  P +
  '" xmlns:a="' +
  A +
  '" xmlns:r="' +
  R +
  '" show="' +
  show +
  '"><p:cSld><p:spTree>' +
  body +
  "</p:spTree></p:cSld></p:sld>";
function parts(
  slides: string[],
  order = slides.map((_, i) => i),
): Record<string, string | Uint8Array> {
  const result: Record<string, string | Uint8Array> = {
    "[Content_Types].xml":
      '<Types xmlns="' +
      T +
      '"><Override PartName="/ppt/presentation.xml" ContentType="' +
      mainType +
      '"/>' +
      slides
        .map(
          (_, i) =>
            '<Override PartName="/ppt/slides/slide' +
            i +
            '.xml" ContentType="' +
            slideType +
            '"/>',
        )
        .join("") +
      "</Types>",
    "_rels/.rels":
      '<Relationships xmlns="' +
      OPC +
      '"><Relationship Id="main" Type="' +
      R +
      '/officeDocument" Target="ppt/presentation.xml"/></Relationships>',
    "ppt/presentation.xml":
      '<p:presentation xmlns:p="' +
      P +
      '" xmlns:r="' +
      R +
      '"><p:sldIdLst>' +
      order
        .map((i) => '<p:sldId id="' + (256 + i) + '" r:id="s' + i + '"/>')
        .join("") +
      "</p:sldIdLst></p:presentation>",
    "ppt/_rels/presentation.xml.rels":
      '<Relationships xmlns="' +
      OPC +
      '">' +
      slides
        .map(
          (_, i) =>
            '<Relationship Id="s' +
            i +
            '" Type="' +
            R +
            '/slide" Target="slides/slide' +
            i +
            '.xml"/>',
        )
        .join("") +
      "</Relationships>",
  };
  // Deliberately reverse physical ZIP entry order.
  for (let i = slides.length - 1; i >= 0; i--)
    result["ppt/slides/slide" + i + ".xml"] = slides[i]!;
  return result;
}
const archive = (files: Record<string, string | Uint8Array>, stored = false) =>
  zipSync(
    Object.fromEntries(
      Object.entries(files).map(([key, value]) => [
        key,
        typeof value === "string" ? strToU8(value) : value,
      ]),
    ),
    { level: stored ? 0 : 6, mtime: new Date("2020-01-01T00:00:00.000Z") },
  );
const visible = (view: Awaited<ReturnType<typeof prepareOfficeReading>>) =>
  view.rows.map((r) => r.text).join("\n");
const digest = async (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
function sample() {
  const table =
    "<p:graphicFrame><a:graphic><a:graphicData><a:tbl><a:tr>" +
    ["North", "42"]
      .map((v) => "<a:tc><a:txBody>" + paragraph(v) + "</a:txBody></a:tc>")
      .join("") +
    "</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>";
  const image =
    '<p:pic><p:nvPicPr><p:cNvPr id="9" name="Image" descr="Growth &amp; outlook"/></p:nvPicPr></p:pic>';
  const files = parts(
    [
      slide(
        shape("Overview Δ 🚀", true) +
          shape("<script>literal</script>") +
          table +
          image,
      ),
      slide(shape("Done"), "0"),
    ],
    [1, 0],
  );
  files["ppt/notesSlides/notesSlide1.xml"] =
    "<notes>Speaker notes are not read</notes>";
  return files;
}
test("PPTX reads presentation order, hidden slides, titles, literal Unicode, tables and image descriptions", async () => {
  for (const stored of [false, true]) {
    const bytes = archive(sample(), stored),
      copy = bytes.slice();
    const view = await prepareOfficeReading("pptx", bytes, () => true);
    expect(visible(view)).toBe(
      "Slide 1 · Hidden\nDone\nSlide 2\nOverview Δ 🚀\n<script>literal</script>\nTable 1 · Row 1 · Cell 1\nNorth\nTable 1 · Row 1 · Cell 2\n42\n[Image: Growth & outlook]",
    );
    expect(
      view.rows.find((r) => r.text.startsWith("Overview"))?.node?.type,
    ).toBe("heading");
    expect(view.note).toContain("Export the original");
    expect(view.note).toContain("notes");
    expect(bytes).toEqual(copy);
    if (!stored && process.env.ION_PPTX_EXPORT_FIXTURE)
      writeFileSync(process.env.ION_PPTX_EXPORT_FIXTURE, bytes);
  }
});
test("strict namespaces, relocated main/slide parts and default content types retain the same reading", async () => {
  let files = parts([slide(shape("Strict"))]);
  files["[Content_Types].xml"] =
    '<Types xmlns="' +
    T +
    '"><Default Extension="XML" ContentType="' +
    slideType +
    '"/><Override PartName="/custom/deck.xml" ContentType="' +
    mainType +
    '"/></Types>';
  files["_rels/.rels"] = String(files["_rels/.rels"]).replace(
    "ppt/presentation.xml",
    "custom/deck.xml",
  );
  files["custom/deck.xml"] = files["ppt/presentation.xml"]!;
  files["custom/_rels/deck.xml.rels"] = String(
    files["ppt/_rels/presentation.xml.rels"],
  ).replace("slides/slide0.xml", "../pages/slide.xml");
  files["pages/slide.xml"] = files["ppt/slides/slide0.xml"]!;
  delete files["ppt/presentation.xml"];
  delete files["ppt/_rels/presentation.xml.rels"];
  delete files["ppt/slides/slide0.xml"];
  files = Object.fromEntries(
    Object.entries(files).map(([key, value]) => [
      key,
      String(value)
        .replaceAll(P, "http://purl.oclc.org/ooxml/presentationml/main")
        .replaceAll(A, "http://purl.oclc.org/ooxml/drawingml/main")
        .replaceAll(
          R,
          "http://purl.oclc.org/ooxml/officeDocument/relationships",
        ),
    ]),
  );
  expect(
    visible(await prepareOfficeReading("pptx", archive(files), () => true)),
  ).toBe("Slide 1\nStrict");
});
test("an empty presentation has an explicit reading state", async () => {
  const files = parts([]);
  delete files["ppt/_rels/presentation.xml.rels"];
  expect(
    visible(await prepareOfficeReading("pptx", archive(files), () => true)),
  ).toBe("This presentation has no slides.");
});
test("PPTX format and required content types refuse macro-enabled or mismatched Office parts", async () => {
  for (const type of [
    "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
  ]) {
    const files = sample();
    files["[Content_Types].xml"] = String(files["[Content_Types].xml"]).replace(
      mainType,
      type,
    );
    await expect(
      prepareOfficeReading("pptx", archive(files), () => true),
    ).rejects.toThrow("format");
  }
  await expect(
    prepareOfficeReading("docx", archive(sample()), () => true),
  ).rejects.toThrow("format");
});
test("required slide relationships refuse external, absent, escaping, wrong-type and duplicate targets", async () => {
  for (const [from, to] of [
    [
      'Target="slides/slide1.xml"',
      'Target="https://example.com/slide.xml" TargetMode="External"',
    ],
    ['Target="slides/slide1.xml"', 'Target="../../outside.xml"'],
    ['Target="slides/slide1.xml"', 'Target="slides/missing.xml"'],
    [
      '/slide" Target="slides/slide1.xml"',
      '/worksheet" Target="slides/slide1.xml"',
    ],
    ['Target="slides/slide1.xml"', 'Target="slides/slide0.xml"'],
  ]) {
    const files = sample(),
      path = "ppt/_rels/presentation.xml.rels";
    files[path] = String(files[path]).replace(from, to);
    await expect(
      prepareOfficeReading("pptx", archive(files), () => true),
    ).rejects.toThrow();
  }
  const files = sample();
  delete files["ppt/_rels/presentation.xml.rels"];
  await expect(
    prepareOfficeReading("pptx", archive(files), () => true),
  ).rejects.toThrow();
});
test("presentation IDs, references and lists must be unique and valid", async () => {
  for (const [from, to] of [
    ['id="257"', 'id="255"'],
    ['id="257"', 'id="256"'],
    ['id="257"', 'id="2147483648"'],
    ['id="257"', 'id="NaN"'],
    ['r:id="s1"', 'r:id="s0"'],
    ['r:id="s1"', 'r:id="missing"'],
    ["</p:sldIdLst>", "</p:sldIdLst><p:sldIdLst/>"],
  ]) {
    const files = sample(),
      path = "ppt/presentation.xml";
    files[path] = String(files[path]).replace(from, to);
    await expect(
      prepareOfficeReading("pptx", archive(files), () => true),
    ).rejects.toThrow();
  }
});
test("wrong slide roots, content types and visibility values refuse extraction", async () => {
  for (const kind of ["root", "visibility", "type", "duplicate-default"]) {
    const files = sample();
    if (kind === "root")
      files["ppt/slides/slide1.xml"] = '<p:notes xmlns:p="' + P + '"/>';
    if (kind === "visibility")
      files["ppt/slides/slide1.xml"] = String(
        files["ppt/slides/slide1.xml"],
      ).replace('show="0"', 'show="maybe"');
    if (kind === "type")
      files["[Content_Types].xml"] = String(
        files["[Content_Types].xml"],
      ).replaceAll(slideType, "application/xml");
    if (kind === "duplicate-default")
      files["[Content_Types].xml"] = String(
        files["[Content_Types].xml"],
      ).replace(
        "</Types>",
        '<Default Extension="xml" ContentType="application/xml"/><Default Extension="XML" ContentType="application/xml"/></Types>',
      );
    await expect(
      prepareOfficeReading("pptx", archive(files), () => true),
    ).rejects.toThrow();
  }
});
test("fallback branches and saved field text are readable while extension/instruction/notes data stays out", async () => {
  const body =
    '<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="x">' +
    shape("Unsupported choice") +
    "</mc:Choice><mc:Fallback>" +
    shape("Fallback") +
    "</mc:Fallback></mc:AlternateContent>" +
    '<p:sp><p:txBody><a:p><a:fld id="f" type="datetime"><a:t>Saved date</a:t></a:fld><a:br/><a:r><a:t>Next</a:t></a:r></a:p></p:txBody></p:sp>' +
    "<p:extLst>" +
    shape("Extension") +
    "</p:extLst>";
  const view = await prepareOfficeReading(
    "pptx",
    archive(parts([slide(body)])),
    () => true,
  );
  expect(visible(view)).toBe("Slide 1\nFallback\nSaved date\nNext");
});
test("slide count, table row/cell counts and extracted text remain bounded", async () => {
  for (const files of [
    parts(Array.from({ length: 257 }, () => slide(shape("x")))),
    parts([slide("<a:tbl>" + "<a:tr/>".repeat(10001) + "</a:tbl>")]),
    parts([
      slide("<a:tbl><a:tr>" + "<a:tc/>".repeat(20001) + "</a:tr></a:tbl>"),
    ]),
    parts([slide(shape("a".repeat(512 * 1024 + 1)))]),
  ])
    await expect(
      prepareOfficeReading("pptx", archive(files), () => true),
    ).rejects.toThrow(/limit|excessive/i);
}, 20_000);
test("revoked scope interrupts indexing and slide preparation without changing the original", async () => {
  const bytes = archive(
    parts(Array.from({ length: 10 }, () => slide(shape("Δ 🚀 ".repeat(1000))))),
  );
  const copy = bytes.slice();
  for (const limit of [1, 20, 100]) {
    let checks = 0;
    await expect(
      prepareOfficeReading("pptx", bytes, () => ++checks < limit),
    ).rejects.toThrow(/access changed|scope/i);
  }
  expect(bytes).toEqual(copy);
});
function item(bytes: Uint8Array): ArtifactLibraryItem {
  return {
    id: randomUUID(),
    name: "Review.pptx",
    format: "pptx",
    projectId: randomUUID(),
    taskId: randomUUID(),
    createdAt: 100,
    bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    origin: "teamAssignment",
  };
}
test("direct PPTX reads verify immutable metadata/digest before reading and retain original MIME/export", async () => {
  const bytes = archive(sample()),
    expected = item(bytes);
  validateArtifactItem(expected);
  const reply = {
    type: "artifactDocument" as const,
    document: {
      document: expected,
      bytes: Buffer.from(bytes).toString("base64"),
    },
  };
  const client = { request: async () => reply } as Pick<WakuClient, "request">;
  const original = await readArtifactDocument(
    client,
    expected,
    () => true,
    digest,
  );
  expect(await artifactText(expected, original, () => true)).toBeNull();
  expect(
    visible(await prepareOfficeReading(expected.format, original, () => true)),
  ).toContain("Overview Δ 🚀");
  expect(workflowDocumentMime("pptx")).toBe(
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  );
  expect(workflowExportName(expected, "original")).toEndWith(".pptx");
  expect(original).toEqual(bytes);
  await expect(
    readArtifactDocument(
      client,
      { ...expected, sha256: "0".repeat(64) },
      () => true,
      digest,
    ),
  ).rejects.toThrow(/metadata|digest/i);
});
test("gateway PPTX reads retain chunk identity, expiry/grant checks and the complete original", async () => {
  const files = sample();
  files["ppt/media/unread.bin"] = new Uint8Array(40000).fill(7);
  const bytes = archive(files, true),
    record = item(bytes),
    run = randomUUID();
  const document: WorkflowDocument = {
    id: record.id,
    name: record.name,
    format: record.format,
    projectId: record.projectId,
    taskId: record.taskId,
    bytes: record.bytes,
    sha256: record.sha256,
    createdAt: record.createdAt,
    stage: 0,
    teamRunId: randomUUID(),
    assignmentId: randomUUID(),
  };
  const pairing: GatewayPairing = {
    id: randomUUID(),
    baseUrl: "https://gateway.example",
    token: "fixture-only",
    grants: ["evidenceRead"],
    expiresAt: Date.now() + 120000,
  };
  const offsets: number[] = [];
  const reader = async (query: PortfolioQuery): Promise<PortfolioResult> => {
    if (query.kind !== "workflowDocument") throw new Error("Unexpected query");
    offsets.push(query.query.offset);
    const end = Math.min(query.query.offset + query.query.limit, bytes.length);
    return {
      kind: "workflowDocument",
      page: {
        runId: run,
        document,
        offset: query.query.offset,
        nextOffset: end < bytes.length ? end : null,
        data: Buffer.from(bytes.subarray(query.query.offset, end)).toString(
          "base64",
        ),
      },
      pageTruncated: false,
      servedAt: Date.now(),
    };
  };
  const original = await readGatewayWorkflowDocument(
    pairing,
    randomUUID(),
    run,
    document,
    () => true,
    randomUUID,
    digest,
    reader,
  );
  expect(offsets.length).toBeGreaterThan(2);
  expect(original).toEqual(bytes);
  expect(
    visible(await prepareOfficeReading("pptx", original, () => true)),
  ).toContain("Slide 1 · Hidden");
  await expect(
    readGatewayWorkflowDocument(
      { ...pairing, grants: [] },
      randomUUID(),
      run,
      document,
      () => true,
      randomUUID,
      digest,
      reader,
    ),
  ).rejects.toThrow();
  await expect(
    readGatewayWorkflowDocument(
      { ...pairing, expiresAt: 1 },
      randomUUID(),
      run,
      document,
      () => true,
      randomUUID,
      digest,
      reader,
    ),
  ).rejects.toThrow();
});
test.skipIf(!process.env.ION_PPTX_NATIVE_FIXTURE)(
  "actual native publication/export survives phone immutable verification and slide reading",
  async () => {
    const reply = JSON.parse(
      readFileSync(process.env.ION_PPTX_NATIVE_FIXTURE!, "utf8"),
    ) as {
      type: "artifactDocument";
      document: { document: ArtifactLibraryItem; bytes: string };
    };
    const client = { request: async () => reply } as Pick<
      WakuClient,
      "request"
    >;
    const original = await readArtifactDocument(
      client,
      reply.document.document,
      () => true,
      digest,
    );
    expect(reply.document.document.format).toBe("pptx");
    expect(reply.document.document.origin).toBe("teamAssignment");
    const source = readFileSync(process.env.ION_PPTX_SOURCE_FIXTURE!);
    expect(original).toEqual(new Uint8Array(source));
    expect(
      visible(await prepareOfficeReading("pptx", original, () => true)),
    ).toContain("Overview Δ 🚀");
    expect(await digest(original)).toBe(reply.document.document.sha256);
  },
);
