import { expect, test } from "bun:test";
import { reconcileRuntime } from "./reconcile-runtime";
test("a VM power cycle restarts the existing container even if the saved row said running", async () => {
  const calls: string[][] = [];
  const state = await reconcileRuntime("a".repeat(32), true, async args => { calls.push(args); return args[0] === "inspect" ? "false\n" : "started"; });
  expect(state).toBe("running"); expect(calls[1]).toEqual(["start", "ion-" + "a".repeat(32)]);
  calls.length = 0;
  await reconcileRuntime("a".repeat(32), false, async args => { calls.push(args); return "true\n"; });
  expect(calls[1]).toEqual(["stop", "--time=10", "ion-" + "a".repeat(32)]);
  await expect(reconcileRuntime("a".repeat(32), true, async () => { throw new Error("missing container"); })).rejects.toThrow("missing");
});
