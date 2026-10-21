import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";
import { generateKeyPair, exportJWK, SignJWT } from "jose";
const pair = await generateKeyPair("RS256");
const jwks = {
  keys: [{ ...(await exportJWK(pair.publicKey)), kid: "local-fixture" }],
};
const token = (subject, client = "M7iCZjpo5bzOVO2E5nrvKzDJ00p3ZWy0") =>
  new SignJWT({ azp: client, scope: "ion:gateway" })
    .setProtectedHeader({ alg: "RS256", kid: "local-fixture" })
    .setSubject(subject)
    .setIssuer("https://dev-amit-ksh.au.auth0.com/")
    .setAudience("https://ion-cloud-api")
    .setIssuedAt()
    .setExpirationTime("5m")
    .sign(pair.privateKey);
const origin = "https://api.build-with-ion.tech";
const workspace = (subject) =>
  createHash("sha256")
    .update(JSON.stringify(["https://dev-amit-ksh.au.auth0.com/", subject]))
    .digest("hex");
const request = (path, method = "GET", body, headers = {}) =>
  new Request(path.startsWith("https:") ? path : origin + path, {
    method,
    headers: {
      ...headers,
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
const controllerToken = "c".repeat(43);
const computePolicy = { instanceId: "computeinstance-fixture", projectId: "project-e04qppz9bc0026zt9y9ky7", microUSDPerHour: 1_000_000,
  maxRuntimeSeconds: 120, idleSeconds: 30, startupSeconds: 30, stopAllowanceSeconds: 120,
  limits: { monthly: 7_000_000, lifetime: 22_000_000, accountLifetime: 22_000_000 } };
const inferencePolicy = { models: [{ id: "fixture-model", contextTokens: 4096, outputTokens: 10, inputMicroUSDPerMillion: 1_000_000,
  outputMicroUSDPerMillion: 1_000_000 }], limits: { monthly: 50000, lifetime: 50000, accountLifetime: 10000 } };
const sandboxImage="10000000-0000-4000-8000-000000000001",sandboxOperation="20000000-0000-4000-8000-000000000001";
const sandboxCapability={provider:"codex",model:"pinned",input_cap:32768,output_cap:4096},sandboxExecution={uid:1000,gid:1000,home:"/home/ion"};
const sandboxPolicy={projectId:"aiproject-fixture123",images:[{target:sandboxImage,image:sandboxImage,kind:"harness",manifest:{version:2,capabilities:[sandboxCapability],execution:sandboxExecution}}],maximumOperationMicroUSD:10000,maxRuntimeSeconds:150,pricingReference:"https://fixture.invalid/pricing",limits:{monthly:50000,lifetime:50000,accountLifetime:50000}};
const sandboxProvider={launches:0,cancels:0,body:null,terminal:false};
const appSessions = new Map();
function instance(directory, configured = false, storage = true, apps = false, sandbox = false) {
  return new Miniflare({
    ...convertV4MiniflareOptions({
      name: "ion-test",
      modules: true,
      scriptPath: process.env.ION_WORKER_TEST_BUNDLE ?? fileURLToPath(new URL("../dist/index.js", import.meta.url)),
      compatibilityDate: "2026-10-19",
      compatibilityFlags: ["nodejs_compat"],
      durableObjects: {
        ACCOUNTS: { className: "AccountRelay", useSQLite: true },
        PLATFORM: { className: "Platform", useSQLite: true },
      },
      r2Buckets: storage ? ["OBJECTS"] : [],
      durableObjectsPersist: join(directory, "sql"),
      r2Persist: join(directory, "objects"),
      outboundService: async (req) => {
        if (sandbox && req.url.startsWith("https://api.tokenfactory.nebius.com/sandboxes/v1/")) {
          assert.equal(req.headers.get("authorization"),"Bearer fixture-inference-key");
          assert.equal(req.headers.get("project"),sandboxPolicy.projectId);
          const path=new URL(req.url).pathname;
          if(path.endsWith("/whoami"))return Response.json({permissions:{spawn_disposable:true,list:true,cancel:true}});
          if(path.endsWith("/instances")){sandboxProvider.launches++;sandboxProvider.body=await req.json();return Response.json({uuid:sandboxOperation},{status:201});}
          if(path.endsWith("/operations/"+sandboxOperation)){
            if(req.method==="DELETE"){sandboxProvider.cancels++;sandboxProvider.terminal=true;return new Response(null,{status:202});}
            return Response.json({uuid:sandboxOperation,kind:"instance",status:sandboxProvider.terminal?"CANCELLED":"EXECUTING",image_uuid:sandboxImage,result_image_uuid:null,metadata:{...sandboxProvider.body}});
          }
          throw new Error("Unexpected sandbox fixture route");
        }
        if (apps && req.url.startsWith("https://backend.composio.dev/api/v3.1/")) {
          assert.equal(req.headers.get("x-api-key"), "fixture-project-key");
          const path = new URL(req.url).pathname;
          if (path === "/api/v3.1/tool_router/session" && req.method === "POST") {
            const config = await req.json();
            assert.equal(config.instant, false);
            assert.equal(config.workbench.enable, false);
            assert.equal(config.proxy_execute.enable, false);
            const id = "trs_" + config.user_id;
            appSessions.set(id, { session_id: id, config });
            return Response.json(appSessions.get(id), { status: 201 });
          }
          if (path === "/api/v3.1/connected_accounts/ca_managed") return Response.json({id:"ca_managed",user_id:"ion_"+workspace("judge"),status:"ACTIVE",toolkit:{slug:"fixture_app"}});
          if (path === "/api/v3.1/tools/FIXTURE_READ") return Response.json({slug:"FIXTURE_READ",version:"20261020",toolkit:{slug:"fixture_app"},input_parameters:{type:"object"}});
          if (path === "/api/v3.1/tools/execute/FIXTURE_READ") {
            const body=await req.json();
            assert.deepEqual(body,{connected_account_id:"ca_managed",user_id:"ion_"+workspace("judge"),version:"20261020",arguments:{query:"exact"}});
            return Response.json({successful:true,log_id:"fixture_worker_log",data:{result:"bounded"}});
          }
          const id = path.split("/")[5];
          if (path.endsWith("/toolkits")) return Response.json({ items: [
            { slug: "fixture_app", name: "Fixture App", composio_managed_auth_schemes: ["OAUTH2"],
              connected_account: { id: "ca_operator", user_id: "operator-personal" }, meta: { description: "Safe catalog" } },
          ], next_cursor: null });
          if (appSessions.has(id)) return Response.json(appSessions.get(id));
          throw new Error("Unexpected app fixture operation");
        }
        if (
          req.url === "https://dev-amit-ksh.au.auth0.com/.well-known/jwks.json"
        )
          return Response.json(jwks);
        if (configured && req.url === "https://api.tokenfactory.nebius.com/v1/chat/completions") {
          assert.equal(req.headers.get("authorization"), "Bearer fixture-inference-key");
          const packet = { id: "fixture", model: "fixture-model", choices: [{ index: 0, delta: { role: "assistant", content: "hello" }, finish_reason: "stop" }],
            usage: { prompt_tokens: 2, completion_tokens: 3, total_tokens: 5 } };
          return new Response("data: " + JSON.stringify(packet) + "\n\ndata: [DONE]\n\n", { headers: { "Content-Type": "text/event-stream" } });
        }
        throw new Error("Unexpected outbound request in offline acceptance");
      },
      bindings: {
        ...(sandbox ? {SANDBOX_POLICY_JSON:JSON.stringify(sandboxPolicy),TOKEN_FACTORY_KEY:"fixture-inference-key"}:{}),
        ...(apps ? { COMPOSIO_PROJECT_KEY: "fixture-project-key" } : {}),
        HOSTING_WORKER_TOKEN: "fixture_" + "x".repeat(43),
        HOSTING_WORKER_ORIGIN: "https://worker.fixture.invalid",
        ...(configured ? { COMPUTE_POLICY_JSON: JSON.stringify(computePolicy), COMPUTE_CONTROLLER_TOKEN: controllerToken,
          TOKEN_FACTORY_POLICY_JSON: JSON.stringify(inferencePolicy), TOKEN_FACTORY_KEY: "fixture-inference-key" } : {}),
        PUBLIC_ORIGIN: origin,
        CONTENT_DOMAIN: "build-with-ion.tech",
        MEMBERS_JSON: JSON.stringify([
          { subject: "owner", role: "owner" },
          { subject: "judge", role: "judge" },
        ]),
      },
    }),
    resourcePersistencePath: directory,
    isolatedResourcePersistencePath: directory,
    telemetry: { enabled: false },
  });
}
test("real Workers SQL: account isolation, single-use pairing and restart durability", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-worker-"));
  let mf = instance(directory);
  const call = async (subject, path, method, body, headers) => {
    const ns = await mf.getDurableObjectNamespace("ACCOUNTS");
    return ns
      .get(ns.idFromName(workspace(subject)))
      .fetch(request(path, method, body, headers));
  };
  try {
    assert.equal(
      (
        await mf.dispatchFetch(origin + "/v1/session", {
          headers: {
            "x-ion-account-subject": "owner",
            "x-ion-workspace": workspace("owner"),
          },
        })
      ).status,
      401,
    );
    assert.equal(
      (await (await mf.dispatchFetch(origin + "/v1/bootstrap")).json()).features
        .speech,
      false,
    );
    assert.equal(
      (
        await mf.dispatchFetch(
          origin + "/models/ion-parakeet-110m-tdt-q8-v1.gguf",
        )
      ).status,
      503,
    );
    assert.equal(
      (
        await mf.dispatchFetch(
          origin + "/models/ion-parakeet-110m-tdt-q8-v1.gguf",
          { method: "POST" },
        )
      ).status,
      405,
    );
    assert.equal(
      (
        await mf.dispatchFetch(origin + "/v1/session", {
          headers: { Authorization: "Bearer " + (await token("owner")) },
        })
      ).status,
      200,
    );
    assert.equal(
      (
        await mf.dispatchFetch(origin + "/v1/session", {
          headers: { Authorization: "Bearer " + (await token("outsider")) },
        })
      ).status,
      403,
    );
    assert.equal(
      (
        await mf.dispatchFetch(origin + "/v1/session", {
          headers: {
            Authorization:
              "Bearer " + (await token("owner", "unapproved-client")),
          },
        })
      ).status,
      401,
    );
    const manifest = await (
      await mf.dispatchFetch(
        origin + "/models/ion-parakeet-110m-tdt-q8-v1.json",
      )
    ).json();
    assert.equal(manifest.bytes, 146029600);
    assert.equal(manifest.decoder, "tdt");
    assert.equal(manifest.url, origin + "/models/" + manifest.filename);
    assert.match(manifest.attribution, /NVIDIA and Suno/);
    const invited = await call("owner", "/v1/owner/invites", "POST", {
      grants: ["inspect", "taskRead"],
    });
    assert.equal(invited.status, 201, await invited.clone().text());
    const { invite } = await invited.json();
    assert.equal(
      (
        await call("judge", "/v1/pair", "POST", {
          invite,
          name: "Wrong account",
        })
      ).status,
      403,
    );
    const pair = await call("owner", "/v1/pair", "POST", {
      invite,
      name: "Phone",
    });
    assert.equal(pair.status, 201, await pair.clone().text());
    const device = await pair.json();
    assert.equal(
      (await call("owner", "/v1/pair", "POST", { invite, name: "Replay" }))
        .status,
      403,
    );
    await mf.dispose();
    mf = instance(directory);
    const headers = {
      "X-Waku-Device-Id": device.id,
      "X-Ion-Device-Token": device.token,
    };
    assert.equal(
      (
        await mf.dispatchFetch(origin + "/v1/gateway/v1/device/session", {
          headers: {
            ...headers,
            Authorization: "Bearer " + (await token("owner")),
          },
        })
      ).status,
      200,
    );
    assert.equal(
      (await call("judge", "/v1/device/session", "GET", undefined, headers))
        .status,
      401,
    );
  } finally {
    await mf.dispose();
    rmSync(directory, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  }
});

test("batched relay reads: public JWT workspace selection and revocation remain authoritative", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-worker-relay-batch-"));
  let mf = instance(directory);
  const call = async (subject, path, method = "GET", body, device) => mf.dispatchFetch(origin + "/v1/gateway" + path, {
    method,
    headers: { Authorization: "Bearer " + await token(subject), "Content-Type": "application/json",
      "x-ion-account-subject": "owner", "x-ion-workspace": workspace("owner"),
      ...(device ? { "X-Waku-Device-Id": device.id, "X-Ion-Device-Token": device.token } : {}) },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  try {
    const invited = await call("owner", "/v1/owner/invites", "POST", { grants: ["publish"] });
    assert.equal(invited.status, 201);
    const paired = await call("owner", "/v1/pair", "POST", { invite: (await invited.json()).invite, name: "Batch fixture" });
    assert.equal(paired.status, 201);
    const publisher = await paired.json();
    for (const path of ["/v1/device/sync", "/v1/device/tasks/sync"]) {
      const own = await call("owner", path, "GET", undefined, publisher);
      assert.equal(own.status, 200, await own.clone().text());
      assert.equal((await own.json()).publisherId, publisher.id);
      // Forged hints and an otherwise valid device token cannot choose the
      // owner's database under a judge's JWT, or bypass the JWT altogether.
      assert.equal((await call("judge", path, "GET", undefined, publisher)).status, 401);
      assert.equal((await mf.dispatchFetch(origin + "/v1/gateway" + path, {
        headers: { "X-Waku-Device-Id": publisher.id, "X-Ion-Device-Token": publisher.token },
      })).status, 401);
    }
    assert.equal((await call("owner", "/v1/device/tasks/reconcile", "POST", { activeIds: [] }, publisher)).status, 200);
    await mf.dispose(); mf = instance(directory);
    const restarted = await call("owner", "/v1/device/tasks/sync", "GET", undefined, publisher);
    assert.equal(restarted.status, 200);
    assert.equal((await restarted.json()).snapshotPresent, true);
    assert.equal((await call("owner", `/v1/owner/devices/${publisher.id}/revoke`, "POST")).status, 200);
    for (const path of ["/v1/device/sync", "/v1/device/tasks/sync"])
      assert.equal((await call("owner", path, "GET", undefined, publisher)).status, 401);
  } finally {
    await mf.dispose();
    assert.equal(resolve(directory, ".."), resolve(tmpdir()));
    assert.ok(directory.startsWith(join(tmpdir(), "ion-worker-relay-batch-")));
    rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
});
test("real Workers SQL/R2: owner switches, private publication, idempotency and static serving while compute is off", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-worker-"));
  let mf = instance(directory);
  const call = async (subject, path, method, body) => {
    const ns = await mf.getDurableObjectNamespace("PLATFORM");
    return ns.get(ns.idFromName("platform-v1")).fetch(
      request(path, method, body, {
        "x-ion-account-subject": subject,
        "x-ion-workspace": workspace(subject),
      }),
    );
  };
  try {
    const initial = await call("judge", "/v1/services/controls");
    assert.equal(initial.status, 200, await initial.clone().text());
    assert.deepEqual(await initial.json(), {
      version: 1,
      cloud: false,
      tokenFactory: false,
      appActions: false,
      updated: 0,
      cloudAvailable: false,
      tokenFactoryAvailable: false,
      appActionsAvailable: false,
      canManage: false,
    });
    assert.equal(
      (await call("judge", "/v1/services/controls", "PUT", { cloud: true }))
        .status,
      403,
    );
    const html = Buffer.from(
      "<!doctype html><title>Ion fixture</title><h1>Published by Ion</h1>",
    );
    const body = {
      sourceId: "artifact-fixture",
      title: "HTML fixture",
      kind: "artifact",
      visibility: "private",
      idempotencyKey: "fixture-publication-key-001",
      settings: {
        runtime: "document",
        entry: "index.html",
        build: [],
        start: [],
        outputDirectory: "dist",
        port: 3000,
        healthPath: "/",
      },
      files: [
        {
          path: "index.html",
          bytes: html.length,
          content: html.toString("base64"),
          sha256: createHash("sha256").update(html).digest("hex"),
        },
      ],
    };
    const bootstrap = await (await mf.dispatchFetch(origin + "/v1/bootstrap")).json();
    assert.equal(bootstrap.features.appDeployment, true);
    const js = Buffer.from('document.querySelector("h1").textContent="Static app"');
    const staticBody = { ...body, kind: "project", sourceId: "static-project",
      visibility: "public", idempotencyKey: "static-project-publication-001",
      settings: { ...body.settings, runtime: "static", spa: true },
      files: [...body.files, { path: "assets/app.js", bytes: js.length,
        content: js.toString("base64"), sha256: createHash("sha256").update(js).digest("hex") }] };
    const staticResponse = await call("judge", "/v1/hosting/releases", "POST", staticBody);
    assert.equal(staticResponse.status, 201, await staticResponse.clone().text());
    const staticSite = await staticResponse.json();
    assert.equal(staticSite.releases[0].status, "ready");
    assert.equal(await (await mf.dispatchFetch(staticSite.url + "assets/app.js")).text(), js.toString());
    assert.equal(await (await mf.dispatchFetch(staticSite.url + "settings/profile")).text(), html.toString());
    assert.equal((await mf.dispatchFetch(staticSite.url + "missing.js")).status, 404);
    assert.equal((await mf.dispatchFetch(staticSite.url, { method: "POST" })).status, 405);
    for (const settings of [
      { ...staticBody.settings, build: ["npm", "run", "build"] },
      { ...staticBody.settings, start: ["node", "server.js"] },
      ...["node", "bun", "python"].map(runtime => ({ ...staticBody.settings, runtime, start: [runtime, "index.html"] })),
    ]) {
      assert.equal((await call("judge", "/v1/hosting/releases", "POST", { ...staticBody, settings })).status, 403);
    }
    assert.equal((await call("judge", "/v1/hosting/releases", "POST", { ...staticBody, environment: { SECRET: "not-for-client-code" } })).status, 400);
    const response = await call("judge", "/v1/hosting/releases", "POST", body);
    assert.equal(response.status, 201, await response.clone().text());
    const site = await response.json();
    assert.equal((await mf.dispatchFetch(site.url)).status, 403);
    assert.equal(
      (await call("judge", "/v1/hosting/releases", "POST", body)).status,
      200,
    );
    assert.equal(
      (
        await call("judge", "/v1/hosting/releases", "POST", {
          ...body,
          title: "Changed",
        })
      ).status,
      409,
    );
    assert.equal(
      (await call("owner", "/v1/hosting/sites/" + site.id)).status,
      404,
    );
    assert.equal(
      (
        await call(
          "judge",
          "/v1/hosting/sites/" + site.id + "/visibility",
          "POST",
          { visibility: "public" },
        )
      ).status,
      200,
    );
    const content = await mf.dispatchFetch(site.url);
    assert.equal(content.status, 200);
    assert.equal(await content.text(), html.toString());
    const dynamic = {
      ...body,
      kind: "project",
      sourceId: "app-fixture",
      idempotencyKey: "fixture-dynamic-key-001",
      settings: {
        ...body.settings,
        runtime: "node",
        start: ["node", "index.html"],
      },
    };
    assert.equal(
      (await call("judge", "/v1/hosting/releases", "POST", dynamic)).status,
      403,
    );
    assert.equal(
      (
        await call("owner", "/v1/services/controls", "PUT", {
          tokenFactory: true,
        })
      ).status,
      503,
    );
    assert.equal(
      (await call("owner", "/v1/services/controls", "PUT", { cloud: true }))
        .status,
      503,
    );
    assert.equal(
      (
        await call("owner", "/v1/services/controls", "PUT", {
          tokenFactory: false,
        })
      ).status,
      200,
    );
    await mf.dispose();
    mf = instance(directory);
    const saved = await (await call("judge", "/v1/services/controls")).json();
    assert.equal(saved.tokenFactory, false);
    assert.equal(saved.cloud, false);
    assert.equal((await mf.dispatchFetch(site.url)).status, 200);
    assert.equal(
      (
        await call(
          "judge",
          "/v1/hosting/sites/" + site.id + "/stop",
          "POST",
          {},
        )
      ).status,
      200,
    );
    assert.equal((await mf.dispatchFetch(site.url)).status, 403);
  } finally {
    await mf.dispose();
    rmSync(directory, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  }
});

test("configured pilot: shared inference, durable spending and denied server publishing never wakes compute", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-configured-"));
  let mf = instance(directory, true);
  const dispatch = async (req) => mf.dispatchFetch(req.url, { method: req.method, headers: Object.fromEntries(req.headers),
    body: ["GET", "HEAD"].includes(req.method) ? undefined : await req.text() });
  const call = async (subject, path, method = "GET", body) => dispatch(request(path, method, body, { Authorization: "Bearer " + await token(subject) }));
  const tick = (state) => dispatch(request("/internal/compute/tick", "POST", { instanceId: computePolicy.instanceId,
    projectId: computePolicy.projectId, controllerId: "controller-" + "x".repeat(24), state, operationSettled: true }, { Authorization: "Bearer " + controllerToken }));
  try {
    assert.equal((await dispatch(request("/internal/compute/tick", "POST", {}))).status, 401);
    assert.equal((await tick("STOPPED")).status, 200);
    assert.equal((await call("owner", "/v1/services/controls", "PUT", { cloud: true, tokenFactory: true })).status, 200);
    assert.equal((await call("judge", "/v1/services/controls", "PUT", { cloud: false })).status, 403);
    const catalog = await (await call("judge", "/v1/inference/models")).json();
    assert.equal(catalog.data[0].id, "fixture-model");
    const inference = await call("judge", "/v1/inference/chat/completions", "POST", { model: "fixture-model", messages: [{ role: "user", content: "hello" }], stream: true });
    assert.equal(inference.status, 200, await inference.clone().text());
    assert.match(await inference.text(), /\[DONE\]/);
    assert.equal((await (await call("judge", "/v1/services/usage")).json()).tokenFactory.account.spent, 5);
    const source = Buffer.from('require("http").createServer((q,r)=>r.end("ok")).listen(3000)');
    const publish = await call("judge", "/v1/hosting/releases", "POST", { sourceId: "node-app", title: "App fixture", kind: "project", visibility: "private",
      idempotencyKey: "node-app-publication-key-001", settings: { runtime: "node", entry: "server.js", build: [], start: ["node", "server.js"], outputDirectory: "dist", port: 3000, healthPath: "/" },
      files: [{ path: "server.js", bytes: source.length, content: source.toString("base64"), sha256: createHash("sha256").update(source).digest("hex") }] });
    assert.equal(publish.status, 403, await publish.clone().text());
    assert.equal((await (await tick("STOPPED")).json()).action, "observe");
    assert.equal((await (await call("owner", "/v1/services/usage")).json()).cloud.total.held, 0);
    assert.equal((await call("owner", "/v1/services/controls", "PUT", { cloud: false, tokenFactory: false })).status, 200);
    assert.equal((await (await tick("STOPPED")).json()).action, "observe");
    await mf.dispose(); mf = instance(directory, true);
    const usage = await (await call("judge", "/v1/services/usage")).json();
    assert.equal(usage.tokenFactory.account.spent, 5); assert.equal(usage.cloud.total.held, 0);
    assert.equal((await call("judge", "/v1/inference/models")).status, 403);
  } finally { await mf.dispose(); rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); }
});

// Exercise a real Worker without an object-storage binding, not a mocked bucket.
test("gateway-only deployment preserves auth and inference without advertising storage", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-gateway-only-"));
  const mf = instance(directory, true, false);
  const call = async (subject, path, method = "GET", body) => mf.dispatchFetch(origin + path, {
    method, headers: { Authorization: "Bearer " + await token(subject),
      ...(body ? { "Content-Type": "application/json" } : {}) },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  try {
    const bootstrap = await (await mf.dispatchFetch(origin + "/v1/bootstrap")).json();
    assert.equal(bootstrap.features.htmlPublishing, false);
    assert.equal(bootstrap.features.appDeployment, false);
    assert.equal(bootstrap.features.sharedInference, true);
    assert.equal((await mf.dispatchFetch(origin + "/v1/session")).status, 401);
    assert.equal((await call("owner", "/v1/session")).status, 200);
    assert.equal((await call("outsider", "/v1/session")).status, 403);
    assert.equal((await call("owner", "/v1/hosting/sites")).status, 503);
    assert.equal((await mf.dispatchFetch(origin + "/models/ion-parakeet-110m-tdt-q8-v1.gguf")).status, 503);
    assert.equal((await call("owner", "/v1/services/controls", "PUT", { tokenFactory: true })).status, 200);
    const response = await call("judge", "/v1/inference/chat/completions", "POST", {
      model: "fixture-model", messages: [{ role: "user", content: "hello" }], stream: true,
    });
    assert.equal(response.status, 200);
    assert.match(await response.text(), /\[DONE\]/);
    const usage = await (await call("judge", "/v1/services/usage")).json();
    assert.equal(usage.tokenFactory.account.spent, 5);
  } finally { await mf.dispose(); rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); }
});
test("managed apps: verified JWT scopes session identity, hides operator connections and persists sessions", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-managed-apps-"));
  let mf = instance(directory, false, false, true);
  const call = async (subject, body = {}) => mf.dispatchFetch(origin + "/v1/apps/catalog", {
    method: "POST", headers: { Authorization: "Bearer " + await token(subject), "Content-Type": "application/json",
      "x-ion-account-subject": "owner", "x-ion-workspace": workspace("owner") }, body: JSON.stringify(body),
  });
  try {
    assert.equal((await mf.dispatchFetch(origin + "/v1/apps/catalog", { headers: { "x-ion-workspace": workspace("owner") } })).status, 401);
    assert.equal((await call("unapproved")).status, 403);
    assert.equal((await call("judge", { user_id: "operator-personal" })).status, 400);
    const a = await call("owner"), b = await call("judge");
    assert.equal(a.status, 200, await a.clone().text());
    assert.equal(b.status, 200, await b.clone().text());
    assert.ok(appSessions.has("trs_ion_" + workspace("owner")));
    assert.ok(appSessions.has("trs_ion_" + workspace("judge")));
    const text = await b.text();
    for (const secret of ["ca_operator", "operator-personal", "fixture-project-key", "trs_ion_"]) assert.ok(!text.includes(secret));
    assert.equal((await (await mf.dispatchFetch(origin + "/v1/bootstrap")).json()).features.managedApps, true);
    const size = appSessions.size;
    await mf.dispose(); mf = instance(directory, false, false, true);
    assert.equal((await call("judge")).status, 200);
    assert.equal(appSessions.size, size);
  } finally { await mf.dispose(); rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); }
});


test("managed app effects: public JWT isolation, owner stop, exact approval and durable single dispatch",async()=>{
 const directory=mkdtempSync(join(tmpdir(),"ion-worker-actions-"));let mf=instance(directory,false,false,true);
 const call=async(subject,path,method="POST",body={})=>mf.dispatchFetch(origin+path,{method,headers:{Authorization:"Bearer "+await token(subject),"Content-Type":"application/json"},...(method==="GET"?{}:{body:JSON.stringify(body)})});
 const root="/v1/apps/actions/";
 try{
  const effect={accountId:"ca_managed",action:"FIXTURE_READ",version:"20261020",arguments:{query:"exact"}};
  assert.equal((await call("owner",root+"prepare","POST",effect)).status,404);
  const prepared=await call("judge",root+"prepare","POST",effect);assert.equal(prepared.status,200,await prepared.clone().text());
  const {proposalId}=await prepared.json();
  assert.equal((await call("owner",root+"approve","POST",{proposalId,allow:true})).status,404);
  assert.equal((await call("judge",root+"approve","POST",{proposalId,allow:true})).status,200);
  assert.equal((await call("judge",root+"execute","POST",{proposalId})).status,403);
  assert.equal((await call("judge","/v1/services/controls","PUT",{appActions:true})).status,403);
  assert.equal((await call("owner","/v1/services/controls","PUT",{appActions:true})).status,200);
  const receipt=await call("judge",root+"execute","POST",{proposalId});assert.equal(receipt.status,200,await receipt.clone().text());
  assert.equal((await receipt.json()).log_id,"fixture_worker_log");
  await mf.dispose();mf=instance(directory,false,false,true);
  assert.equal((await (await call("judge",root+"execute","POST",{proposalId})).json()).log_id,"fixture_worker_log");
  const usage=await(await call("judge","/v1/services/usage","GET")).json();assert.equal(usage.combined.total.held,1000);
  assert.equal((await call("owner","/v1/services/controls","PUT",{appActions:false})).status,200);
 }finally{await mf.dispose();rmSync(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100});}
});


test("managed sandbox AI project: authenticated bridge survives restart and alarm confirms owner stop",async()=>{
 const directory=mkdtempSync(join(tmpdir(),"ion-worker-sandbox-"));let mf=instance(directory,false,false,false,true);
 const call=async(subject,path,method="GET",body)=>mf.dispatchFetch(origin+path,{method,headers:{Authorization:"Bearer "+await token(subject),"Content-Type":"application/json"},...(body?{body:JSON.stringify(body)}:{})});
 try{
  const configResponse=await call("judge","/v1/sandbox/config"),configBody=await configResponse.text();assert.equal(configResponse.status,200,configBody);
  const config=JSON.parse(configBody);assert.equal(config.available,true);assert.equal(config.enabled,false);assert.equal(config.project,sandboxPolicy.projectId);
  assert.equal((await call("judge","/v1/services/controls","PUT",{tokenFactory:true})).status,403);
  assert.equal((await call("owner","/v1/services/controls","PUT",{tokenFactory:true})).status,200);
  const id="30000000-0000-4000-8000-000000000001";
  const body={image:sandboxImage,hostname:"ion-"+id,command:"/usr/local/bin/ion-harness-worker",args:[],shell:false,env:{PATH:"/usr/local/bin:/usr/bin:/bin",HOME:"/home/ion"},cwd:"/ion/work",uid:1000,gid:1000,stdin:{value:Buffer.from(JSON.stringify({version:1,request_id:id,capability:sandboxCapability,execution:sandboxExecution,system:"Test",messages:[],tools:[]})).toString("base64"),encoding:"base64",close:true},timeout:150,truncate_output_at:65536,disposable:true,preserve_env:false,resources_limits:{max_layer_bytes:268435456},networking:{enabled:true}};
  const start=await call("judge","/v1/sandbox/instances","POST",body);assert.equal(start.status,201,await start.clone().text());
  assert.equal((await call("owner","/v1/sandbox/operations/"+sandboxOperation)).status,404);
  await mf.dispose();mf=instance(directory,false,false,false,true);
  assert.equal((await call("judge","/v1/sandbox/instances","POST",body)).status,201);assert.equal(sandboxProvider.launches,1);
  assert.equal((await call("owner","/v1/services/controls","PUT",{tokenFactory:false})).status,200);
  const deadline=Date.now()+12000;
  while(!sandboxProvider.terminal && Date.now()<deadline)await new Promise(resolve=>setTimeout(resolve,500));
  assert.equal(sandboxProvider.terminal,true);assert.equal(sandboxProvider.cancels,1);
  assert.equal((await(await call("judge","/v1/sandbox/operations/"+sandboxOperation)).json()).status,"CANCELLED");
  assert.equal((await(await call("judge","/v1/services/usage")).json()).sandbox.account.held,10000);
 }finally{await mf.dispose();rmSync(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100});}
});
