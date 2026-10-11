import {
  sameCloudBudget,
  validateCloudBudget,
  type CloudBudget,
  type WakuClient,
} from "@waku/client";
export interface BudgetDraft {
  version: 1;
  revision: number;
  budget: CloudBudget;
}
export interface BudgetStorage {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
}
export function budgetDraftKey(owner: string) {
  return `ion-gpu-budget-draft:${encodeURIComponent(owner)}`;
}
export function readBudgetDraft(text: string): BudgetDraft {
  const draft = JSON.parse(text);
  if (
    !draft ||
    Object.keys(draft).sort().join(",") !== "budget,revision,version" ||
    draft.version !== 1 ||
    !Number.isSafeInteger(draft.revision) ||
    draft.revision < 0
  )
    throw new Error("Saved GPU budget draft is invalid.");
  validateCloudBudget(draft.budget);
  return draft;
}
/** Persist identity before transport; reconcile exact state after an interrupted save. */
export async function saveBudgetDraft(
  client: WakuClient,
  storage: BudgetStorage,
  key: string,
  draft: BudgetDraft,
  authorized: () => boolean = () => true,
): Promise<void> {
  const requireOwner = () => {
    if (!authorized())
      throw new Error(
        "Return to this owner connection and refresh before saving.",
      );
  };
  requireOwner();
  readBudgetDraft(JSON.stringify(draft));
  await storage.setItem(key, JSON.stringify(draft));
  requireOwner();
  const latest = await client.request({ type: "loadWorkspaceControl" });
  if (latest.type !== "workspaceControl")
    throw new Error("Unexpected workspace response.");
  requireOwner();
  if (
    sameCloudBudget(
      latest.snapshot.records.find((r) => r.id === draft.budget.id),
      draft.budget,
    )
  ) {
    await storage.removeItem(key);
    return;
  }
  if (latest.snapshot.revision !== draft.revision)
    throw new Error(
      "Workspace changed. Refresh and review the saved budget before replacing this draft.",
    );
  try {
    const saved = await client.request({
      type: "saveWorkspaceRecord",
      expectedRevision: draft.revision,
      record: { kind: "cloudBudget", ...draft.budget },
    });
    if (
      saved.type !== "workspaceControl" ||
      !sameCloudBudget(
        saved.snapshot.records.find((r) => r.id === draft.budget.id),
        draft.budget,
      )
    )
      throw new Error("GPU budget save is not confirmed.");
  } catch (error) {
    const check = await client
      .request({ type: "loadWorkspaceControl" })
      .catch(() => null);
    if (
      check?.type !== "workspaceControl" ||
      !sameCloudBudget(
        check.snapshot.records.find((r) => r.id === draft.budget.id),
        draft.budget,
      )
    )
      throw error;
  }
  await storage.removeItem(key);
}
