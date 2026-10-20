import { expect, test } from "bun:test";
import { Admission } from "./admission";
import type { Account } from "./auth";
const account = (subject: string): Account => ({
  subject,
  issuer: "https://issuer/",
  workspaceId: "a".repeat(64),
  scopes: new Set(["ion:gateway"]),
});
test("closed admission denies unknown users and never infers roles", () => {
  const policy = new Admission([
    { subject: "auth0|owner", role: "owner" },
    { subject: "auth0|judge", role: "judge" },
  ]);
  expect(policy.admit(account("auth0|judge"))).toBe("judge");
  expect(() => policy.owner(account("auth0|judge"))).toThrow(
    "Owner permission",
  );
  expect(() => policy.owner(account("auth0|owner"))).not.toThrow();
  expect(() => policy.admit(account("auth0|other"))).toThrow("not enabled");
  expect(() => new Admission([]).admit(account("auth0|owner"))).toThrow();
});
test("malformed and duplicate allowlists fail before serving", () => {
  for (const input of [
    null,
    {},
    [{ subject: "a", role: "admin" }],
    [
      { subject: "a", role: "owner" },
      { subject: "a", role: "judge" },
    ],
  ])
    expect(() => new Admission(input)).toThrow();
});
