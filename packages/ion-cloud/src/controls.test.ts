import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Admission } from "./admission";
import { Controls } from "./controls";
import { ServiceStore } from "./service-store";
import type { Account } from "./auth";
const policy = new Admission([
  { subject: "owner", role: "owner" },
  { subject: "judge", role: "judge" },
]);
const account = (subject: string): Account => ({
  subject,
  issuer: "https://issuer/",
  workspaceId: "a".repeat(64),
  scopes: new Set(["ion:gateway"]),
});
const request = (data: unknown) =>
  new Request("https://api.example/v1/services/controls", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  });
test("paid work defaults off; only owner can change switches; state survives restart", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-controls-"));
  let store = new ServiceStore(directory);
  try {
    const unconfigured = new Controls(store, policy);
    await expect(
      unconfigured.handle(account("owner"), request({ cloud: true })),
    ).rejects.toThrow("not configured");
    await expect(
      unconfigured.handle(account("owner"), request({ tokenFactory: true })),
    ).rejects.toThrow("not configured");
    let controls = new Controls(store, policy, {
      cloud: true,
      tokenFactory: true,
    });
    expect(controls.computeEnabled()).toBe(false);
    const get = await controls.handle(
      account("judge"),
      new Request("https://api.example/v1/services/controls"),
    );
    expect(await get.json()).toMatchObject({
      cloud: false,
      tokenFactory: false,
      canManage: false,
    });
    await expect(
      controls.handle(
        account("judge"),
        request({ cloud: true, tokenFactory: true }),
      ),
    ).rejects.toThrow("Owner permission");
    await expect(
      controls.handle(
        account("owner"),
        request({ cloud: "yes", tokenFactory: true }),
      ),
    ).rejects.toThrow("switches");
    await expect(
      controls.handle(
        account("owner"),
        request({ cloud: true, tokenFactory: true, extra: true }),
      ),
    ).rejects.toThrow("switches");
    await controls.handle(
      account("owner"),
      request({ cloud: true, tokenFactory: false }),
    );
    expect(controls.computeEnabled()).toBe(true);
    store.db.close(true);
    store = new ServiceStore(directory);
    controls = new Controls(store, policy, { cloud: true, tokenFactory: true });
    expect(controls.get()).toMatchObject({ cloud: true, tokenFactory: false });
    const off = await controls.handle(
      account("owner"),
      request({ cloud: false, tokenFactory: true }),
    );
    expect(await off.json()).toMatchObject({
      cloud: false,
      tokenFactory: true,
      computeStopPending: true,
    });
  } finally {
    store.close();
    rmSync(directory, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  }
});


test("app actions remain off across upgrades and only the owner can enable a configured service", async () => {
 const directory=mkdtempSync(join(tmpdir(),"ion-app-controls-")), store=new ServiceStore(directory);
 try {
  store.put("_operator","controls","services",{version:1,cloud:false,tokenFactory:false,updated:1});
  const controls=new Controls(store,policy,{cloud:false,tokenFactory:false,appActions:true});
  expect(controls.get().appActions).toBe(false);
  await expect(controls.handle(account("judge"),request({appActions:true}))).rejects.toThrow("Owner permission");
  await expect(new Controls(store,policy).handle(account("owner"),request({appActions:true}))).rejects.toThrow("not configured");
  await controls.handle(account("owner"),request({appActions:true}));
  expect(new Controls(store,policy).get().appActions).toBe(true);
  await controls.handle(account("owner"),request({appActions:false}));
  expect(controls.get().appActions).toBe(false);
 } finally { store.close();rmSync(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100}); }
});
