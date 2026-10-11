import { test, expect } from "bun:test";
import type { ControlSnapshot, WakuClient } from "@waku/client";
import {
  budgetDraftKey,
  readBudgetDraft,
  saveBudgetDraft,
  type BudgetDraft,
} from "./cloud-budget-drafts";
const budget = {
  id: "00000000-0000-4000-8000-000000000001",
  name: "GPU",
  cloudProject: "project-test",
  currency: "USD",
  limitMicros: 1000,
  targets: ["training"],
  paused: false,
};
const draft: BudgetDraft = { version: 1, revision: 4, budget };
function storage() {
  const rows = new Map<string, string>();
  return {
    rows,
    getItem: async (k: string) => rows.get(k) ?? null,
    setItem: async (k: string, v: string) => {
      rows.set(k, v);
    },
    removeItem: async (k: string) => {
      rows.delete(k);
    },
  };
}
function client(load: () => ControlSnapshot, save: (request: any) => unknown) {
  return {
    request: async (r: any) =>
      r.type === "loadWorkspaceControl"
        ? { type: "workspaceControl", snapshot: load() }
        : save(r),
  } as unknown as WakuClient;
}
test("budget draft persists exact identity before a write and survives lost receipt", async () => {
  const store = storage(),
    key = budgetDraftKey("owner");
  let snapshot: ControlSnapshot = { revision: 4, records: [] };
  let writes = 0;
  const c = client(
    () => snapshot,
    (r) => {
      expect(store.rows.has(key)).toBe(true);
      expect(r.record.id).toBe(budget.id);
      writes++;
      snapshot = { revision: 5, records: [{ kind: "cloudBudget", ...budget }] };
      throw new Error("receipt lost");
    },
  );
  await saveBudgetDraft(c, store, key, draft);
  expect(writes).toBe(1);
  expect(store.rows.has(key)).toBe(false);
  await saveBudgetDraft(c, store, key, draft);
  expect(writes).toBe(1);
});
test("unconfirmed write retains the same request for restart without creating another budget", async () => {
  const store = storage(),
    key = budgetDraftKey("owner");
  const c = client(
    () => ({ revision: 4, records: [] }),
    (r) => {
      throw new Error("offline");
    },
  );
  await expect(saveBudgetDraft(c, store, key, draft)).rejects.toThrow(
    "offline",
  );
  expect(readBudgetDraft(store.rows.get(key)!).budget.id).toBe(budget.id);
});
test("stale revision, background or owner switch cannot write a budget", async () => {
  const store = storage();
  let writes = 0;
  let active = true;
  const c = client(
    () => {
      active = false;
      return { revision: 4, records: [] };
    },
    () => {
      writes++;
    },
  );
  await expect(
    saveBudgetDraft(c, store, "key", draft, () => active),
  ).rejects.toThrow("owner connection");
  expect(writes).toBe(0);
  await expect(
    saveBudgetDraft(
      client(
        () => ({ revision: 5, records: [] }),
        () => {
          writes++;
        },
      ),
      store,
      "other",
      draft,
    ),
  ).rejects.toThrow("Workspace changed");
  expect(writes).toBe(0);
});
test("a different saved budget never confirms an interrupted proposal", async () => {
  const store = storage();
  const c = client(
    () => ({
      revision: 4,
      records: [{ kind: "cloudBudget", ...budget, limitMicros: 999 }],
    }),
    () => {
      throw new Error("lost");
    },
  );
  await expect(saveBudgetDraft(c, store, "key", draft)).rejects.toThrow("lost");
  expect(store.rows.has("key")).toBe(true);
  expect(budgetDraftKey("one")).not.toBe(budgetDraftKey("two"));
  expect(() =>
    readBudgetDraft(JSON.stringify({ ...draft, credential: "secret" })),
  ).toThrow();
});
