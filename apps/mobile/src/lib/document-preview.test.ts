import { expect, test } from "bun:test";
import { documentPreviewRows } from "./document-preview";
test("Markdown reading retains headings and removes links and remote images recursively", () => {
  const rows = documentPreviewRows(
    "md",
    "# Title\n\n- [link](javascript:command())\n- ![diagram](https://remote.test/pixel)",
    true,
  );
  expect(rows[0].node?.type).toBe("heading");
  const parsed = JSON.stringify(rows.map((row) => row.node));
  expect(parsed).not.toContain("https://remote.test");
  expect(parsed).not.toContain("javascript:");
  expect(parsed).toContain("Image: diagram");
});
test("source mode preserves Unicode and long blocks stay bounded without losing text", () => {
  const text = "語".repeat(21_000);
  for (const reading of [true, false]) {
    const rows = documentPreviewRows("md", text, reading);
    expect(rows.length).toBeGreaterThan(10);
    expect(rows.every((r) => r.text.length <= 1000)).toBe(true);
    expect(rows.map((r) => r.text).join("")).toBe(text);
  }
  const source = "# Heading\n\n![image](https://remote.test)";
  expect(
    documentPreviewRows("md", source, false)
      .map((r) => r.text)
      .join(""),
  ).toBe(source);
});
test("HTML remains inert source, JSON reading is formatted and invalid JSON remains readable", () => {
  const html = "<script>fetch('https://remote.test')</script>";
  expect(documentPreviewRows("html", html, true)).toEqual([
    { id: "0", text: html, node: null },
  ]);
  expect(documentPreviewRows("json", '{"a":1}', true)[0].text).toBe(
    '{\n  "a": 1\n}',
  );
  expect(documentPreviewRows("json", "{bad", true)[0].text).toBe("{bad");
  expect(() => documentPreviewRows("txt", "語".repeat(22_000), false)).toThrow(
    "limit",
  );
});

test("raw HTML inside Markdown stays readable and inert", () => {
  const rows = documentPreviewRows(
    "md",
    "<script>fetch('https://remote.test')</script>",
    true,
  );
  expect(rows[0].node?.type).toBe("paragraph");
  expect(JSON.stringify(rows[0].node)).toContain("<script>");
});

test("reading text does not require a browser TextEncoder in the native runtime", () => {
  const original = globalThis.TextEncoder;
  try {
    Object.assign(globalThis, { TextEncoder: undefined });
    expect(
      documentPreviewRows("txt", "नमस्ते", true)
        .map((row) => row.text)
        .join(""),
    ).toBe("नमस्ते");
  } finally {
    Object.assign(globalThis, { TextEncoder: original });
  }
});

test("JSON reading retains bounded complete source when formatting would expand it", () => {
  for (const text of [
    "[".repeat(1000) + "1" + "]".repeat(1000),
    "[" + Array(4000).fill("1e20").join(",") + "]",
  ]) {
    const rows = documentPreviewRows("json", text, true);
    expect(rows.map((row) => row.text).join("")).toBe(text);
    expect(rows.every((row) => row.text.length <= 1000)).toBe(true);
  }
});
