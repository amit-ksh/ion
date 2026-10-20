import { expect, test } from "bun:test";
import {
  mkdtempSync,
  mkdirSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { contentChangeFilter } from "./dev-watch";

test("watcher ignores metadata and identical rewrites, but observes changed bytes and entries", () => {
  const root = mkdtempSync(join(tmpdir(), "ion-watch-test-"));
  try {
    writeFileSync(join(root, "app.rs"), "before");
    const changed = contentChangeFilter(root);
    utimesSync(join(root, "app.rs"), new Date(), new Date());
    expect(changed("app.rs")).toBe(false);
    writeFileSync(join(root, "app.rs"), "before");
    expect(changed("app.rs")).toBe(false);
    writeFileSync(join(root, "app.rs"), "after!");
    expect(changed("app.rs")).toBe(true);
    expect(changed("app.rs")).toBe(false);
    mkdirSync(join(root, "nested"));
    writeFileSync(join(root, "nested", "new.rs"), "new");
    expect(changed("nested")).toBe(true);
    renameSync(join(root, "nested"), join(root, "renamed"));
    expect(changed(null)).toBe(true);
    rmSync(join(root, "app.rs"));
    expect(changed("app.rs")).toBe(true);
    expect(changed(null)).toBe(false);
  } finally {
    rmSync(root, { recursive: true });
  }
});
