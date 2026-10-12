import { beforeEach, expect, mock, test } from "bun:test";
let current = true,
  picked = 0,
  created = 0,
  opened = 0,
  closed = 0,
  deleted = 0;
let chunks: Uint8Array[] = [];
let offset = 0;
let trace: string[] = [];
let onWrite = (_bytes: Uint8Array) => {};
let onCreate = () => {};
let onClose = () => {};
const handle = {
  get offset() {
    return offset;
  },
  get size() {
    return offset;
  },
  writeBytes(bytes: Uint8Array) {
    chunks.push(bytes.slice());
    offset += bytes.length;
    onWrite(bytes);
  },
  close() {
    closed++;
    trace.push("close");
    onClose();
  },
};
const file = {
  open(mode: string) {
    expect(mode).toBe("w");
    opened++;
    return handle;
  },
  delete() {
    deleted++;
    trace.push("delete");
  },
};
let picker = async () => ({
  createFile: (_name: string, _mime: string) => {
    created++;
    onCreate();
    return file;
  },
});
mock.module("expo-file-system", () => ({
  FileMode: { WriteOnly: "w" },
  Directory: {
    pickDirectoryAsync: () => {
      picked++;
      return picker();
    },
  },
}));
const { exportWorkflowDocument } =
  await import("./workflow-document-export.native");
const record = {
  id: "11111111-1111-4111-8111-111111111111",
  name: "document.md",
  format: "md",
};
beforeEach(() => {
  current = true;
  picked = created = opened = closed = deleted = offset = 0;
  chunks = [];
  trace = [];
  onWrite = () => {};
  onCreate = () => {};
  onClose = () => {};
  picker = async () => ({
    createFile: () => {
      created++;
      onCreate();
      return file;
    },
  });
});
test("native export refuses stale scope before and after the folder picker", async () => {
  current = false;
  await expect(
    exportWorkflowDocument(record, new Uint8Array([1]), () => current),
  ).rejects.toThrow("scope changed");
  expect(picked).toBe(0);
  current = true;
  picker = async () => {
    current = false;
    return {
      createFile: () => {
        created++;
        return file;
      },
    };
  };
  await expect(
    exportWorkflowDocument(record, new Uint8Array([1]), () => current),
  ).rejects.toThrow("scope changed");
  expect(created).toBe(0);
  expect(chunks).toEqual([]);
});
test("native export bounds original bytes before requesting folder access", async () => {
  for (const bytes of [new Uint8Array(0), new Uint8Array(16 * 1024 * 1024 + 1)])
    await expect(
      exportWorkflowDocument(record, bytes, () => current),
    ).rejects.toThrow("byte limit");
  expect(picked).toBe(0);
});
test("native export writes complete original bytes in bounded chunks and closes", async () => {
  const bytes = Uint8Array.from({ length: 2 * 65_536 + 3 }, (_, i) => i % 251);
  await exportWorkflowDocument(record, bytes, () => current);
  expect(chunks.map((chunk) => chunk.length)).toEqual([65_536, 65_536, 3]);
  const actual = new Uint8Array(bytes.length);
  let at = 0;
  for (const chunk of chunks) {
    actual.set(chunk, at);
    at += chunk.length;
  }
  expect(actual).toEqual(bytes);
  expect(opened).toBe(1);
  expect(closed).toBe(1);
  expect(deleted).toBe(0);
});
test("native export closes before removing partially written output on write failure", async () => {
  onWrite = () => {
    if (chunks.length === 2) throw new Error("disk full after partial write");
  };
  await expect(
    exportWorkflowDocument(record, new Uint8Array(3 * 65_536), () => current),
  ).rejects.toThrow("disk full");
  expect(chunks.length).toBe(2);
  expect(closed).toBe(1);
  expect(trace).toEqual(["close", "delete"]);
});
test("native export yields between chunks so scope cancellation prevents later writes", async () => {
  onWrite = () => {
    if (chunks.length === 1)
      setTimeout(() => {
        current = false;
      }, 0);
  };
  await expect(
    exportWorkflowDocument(record, new Uint8Array(3 * 65_536), () => current),
  ).rejects.toThrow("scope changed");
  expect(chunks.map((chunk) => chunk.length)).toEqual([65_536]);
  expect(trace).toEqual(["close", "delete"]);
});
test("native export rechecks after creation and removes output without opening stale scope", async () => {
  onCreate = () => {
    current = false;
  };
  await expect(
    exportWorkflowDocument(record, new Uint8Array([1]), () => current),
  ).rejects.toThrow("scope changed");
  expect(opened).toBe(0);
  expect(chunks).toEqual([]);
  expect(deleted).toBe(1);
});
test("native export refuses short writes and removes incomplete output", async () => {
  onWrite = () => {
    offset--;
  };
  await expect(
    exportWorkflowDocument(record, new Uint8Array([1, 2]), () => current),
  ).rejects.toThrow("complete original");
  expect(trace).toEqual(["close", "delete"]);
});
test("native export preserves close errors while removing abandoned output", async () => {
  onClose = () => {
    throw new Error("close failed");
  };
  await expect(
    exportWorkflowDocument(record, new Uint8Array([1]), () => current),
  ).rejects.toThrow("close failed");
  expect(trace.at(-1)).toBe("delete");
});
