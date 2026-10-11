import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import { startGateway } from "./server";
const dirs: string[] = [];
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});
const ownerToken = "owner_" + "r".repeat(48);
function setup() {
  const dir = mkdtempSync(join(tmpdir(), "ion-portfolio-http-"));
  dirs.push(dir);
  const gateway = startGateway({
    databasePath: join(dir, "state.sqlite"),
    ownerToken,
    allowedTools: [],
  });
  const owner = { id: "", token: ownerToken };
  async function call(
    path: string,
    method = "GET",
    identity?: { id: string; token: string },
    data?: unknown,
  ) {
    const headers = new Headers();
    if (identity) {
      headers.set("Authorization", `Bearer ${identity.token}`);
      headers.set("X-Waku-Device-Id", identity.id);
    }
    if (data !== undefined) headers.set("Content-Type", "application/json");
    return fetch(new URL(path, gateway.server.url), {
      method,
      headers,
      body: data === undefined ? undefined : JSON.stringify(data),
    });
  }
  async function pair(grants: string[]) {
    const invite = (await (
      await call("/v1/owner/invites", "POST", owner, { grants })
    ).json()) as { invite: string };
    return (await (
      await call("/v1/pair", "POST", undefined, {
        invite: invite.invite,
        name: grants.join(" "),
      })
    ).json()) as { id: string; token: string };
  }
  return { gateway, owner, call, pair };
}
const projects = { kind: "projects", query: { after: null, limit: 32 } };
const inbox = {
  kind: "inbox",
  query: {
    projectId: null,
    taskId: null,
    kind: null,
    view: "all",
    status: "all",
    after: null,
    limit: 32,
  },
};
const usage = {
  kind: "usage",
  query: {
    projectId: null,
    taskId: null,
    agentId: null,
    provider: null,
    environment: null,
    kind: null,
    model: null,
    since: null,
    after: null,
    limit: 32,
  },
};
test("HTTP read grants, exact publisher routing and requester response scope stay independent from controls", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      other = await f.pair(["publish"]),
      reader = await f.pair(["inboxRead"]),
      usageReader = await f.pair(["usageRead"]),
      legacy = await f.pair(["automationRead", "taskRead"]);
    expect((await f.call("/v1/device/session", "GET", reader)).status).toBe(
      200,
    );
    // Workflow readers can now discover history owners, but cannot read Inbox.
    expect((await f.call("/v1/device/portfolio", "GET", legacy)).status).toBe(
      200,
    );
    expect(
      (
        await f.call("/v1/device/portfolio/queries", "POST", legacy, {
          publisherId: publisher.id,
          idempotencyKey: randomUUID(),
          query: inbox,
        })
      ).status,
    ).toBe(401);
    expect(
      (await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 }))
        .status,
    ).toBe(200);
    expect(
      (await f.call("/v1/device/portfolio", "PUT", reader, { version: 1 }))
        .status,
    ).toBe(401);
    const owners = (await (
      await f.call("/v1/device/portfolio", "GET", usageReader)
    ).json()) as { owners: any[] };
    expect(owners.owners.map((o) => o.publisherId)).toEqual([publisher.id]);
    expect(
      (
        await f.call("/v1/device/portfolio/queries", "POST", reader, {
          publisherId: publisher.id,
          idempotencyKey: randomUUID(),
          query: usage,
        })
      ).status,
    ).toBe(401);
    expect(
      (
        await f.call("/v1/device/portfolio/queries", "POST", usageReader, {
          publisherId: publisher.id,
          idempotencyKey: randomUUID(),
          query: inbox,
        })
      ).status,
    ).toBe(401);
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      query: projects,
    };
    const response = await f.call(
      "/v1/device/portfolio/queries",
      "POST",
      reader,
      input,
    );
    expect(response.status).toBe(201);
    const job = (await response.json()) as { id: string };
    expect(
      await (await f.call("/v1/device/portfolio/queue", "GET", other)).json(),
    ).toEqual({ jobs: [] });
    expect(
      (await f.call("/v1/device/portfolio/queue", "GET", reader)).status,
    ).toBe(401);
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${job.id}/complete`,
          "POST",
          other,
          { result: {} },
        )
      ).status,
    ).toBe(404);
    const result = {
      kind: "projects",
      page: { projects: [], nextCursor: null, total: 0 },
      pageTruncated: false,
      servedAt: Date.now(),
    };
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${job.id}/complete`,
          "POST",
          publisher,
          { result },
        )
      ).status,
    ).toBe(200);
    const receipt = await f.call(
      `/v1/device/portfolio/queries?key=${input.idempotencyKey}`,
      "GET",
      reader,
    );
    expect(receipt.headers.get("cache-control")).toBe("no-store");
    expect(((await receipt.json()) as any).job.status).toBe("complete");
    expect(
      await (
        await f.call(
          `/v1/device/portfolio/queries?key=${input.idempotencyKey}`,
          "GET",
          usageReader,
        )
      ).json(),
    ).toEqual({ job: null });
    expect(
      (await f.call("/v1/device/automations/commands", "POST", reader, {}))
        .status,
    ).toBe(401);
  } finally {
    await f.gateway.close();
  }
});
test("revocation before owner response discards the read and hides revoked publishers", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      reader = await f.pair(["usageRead"]);
    await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      query: projects,
    };
    const job = (await (
      await f.call("/v1/device/portfolio/queries", "POST", reader, input)
    ).json()) as { id: string };
    await f.call(`/v1/owner/devices/${reader.id}/revoke`, "POST", f.owner, {});
    expect(
      await (
        await f.call("/v1/device/portfolio/queue", "GET", publisher)
      ).json(),
    ).toEqual({ jobs: [] });
    expect(
      (
        (await f
          .call(
            `/v1/device/portfolio/queries/${job.id}/complete`,
            "POST",
            publisher,
            { result: {} },
          )
          .then((r) => r.json())) as any
      ).status,
    ).toBe("rejected");
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries?key=${input.idempotencyKey}`,
          "GET",
          reader,
        )
      ).status,
    ).toBe(401);
    const otherReader = await f.pair(["inboxRead"]);
    await f.call(
      `/v1/owner/devices/${publisher.id}/revoke`,
      "POST",
      f.owner,
      {},
    );
    expect(
      await (await f.call("/v1/device/portfolio", "GET", otherReader)).json(),
    ).toEqual({ owners: [] });
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${job.id}/complete`,
          "POST",
          publisher,
          { result: {} },
        )
      ).status,
    ).toBe(401);
  } finally {
    await f.gateway.close();
  }
});
test("read requests cannot smuggle commands, paths or unsolicited result fields", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      reader = await f.pair(["inboxRead"]);
    await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    for (const query of [
      { kind: "stop", runId: randomUUID() },
      { kind: "projects", query: { after: null, limit: 32, path: "/secrets" } },
    ])
      expect(
        (
          await f.call("/v1/device/portfolio/queries", "POST", reader, {
            publisherId: publisher.id,
            idempotencyKey: randomUUID(),
            query,
          })
        ).status,
      ).toBe(400);
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      query: projects,
    };
    const job = (await (
      await f.call("/v1/device/portfolio/queries", "POST", reader, input)
    ).json()) as { id: string };
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${job.id}/complete`,
          "POST",
          publisher,
          {
            result: {
              kind: "projects",
              page: {
                projects: [],
                nextCursor: null,
                total: 0,
                path: "/private",
              },
              pageTruncated: false,
              servedAt: Date.now(),
            },
          },
        )
      ).status,
    ).toBe(400);
  } finally {
    await f.gateway.close();
  }
});

test("workflow history and document grants are independent and scoped at every read boundary", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      workflow = await f.pair(["automationRead"]),
      documents = await f.pair(["evidenceRead"]),
      task = await f.pair(["taskRead"]);
    await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    expect((await f.call("/v1/device/portfolio", "GET", workflow)).status).toBe(
      200,
    );
    expect(
      (await f.call("/v1/device/portfolio", "GET", documents)).status,
    ).toBe(200);
    expect((await f.call("/v1/device/portfolio", "GET", task)).status).toBe(
      401,
    );
    const run = randomUUID();
    const history = {
      kind: "workflowHistory",
      query: {
        projectId: null,
        definitionId: null,
        pendingOnly: false,
        after: null,
        limit: 32,
      },
    };
    const evidence = {
      kind: "workflowEvidence",
      query: { runId: run, after: null, limit: 32 },
    };
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      query: history,
    };
    expect(
      (await f.call("/v1/device/portfolio/queries", "POST", documents, input))
        .status,
    ).toBe(401);
    expect(
      (
        await f.call("/v1/device/portfolio/queries", "POST", workflow, {
          ...input,
          query: evidence,
        })
      ).status,
    ).toBe(401);
    const job = (await (
      await f.call("/v1/device/portfolio/queries", "POST", workflow, input)
    ).json()) as { id: string };
    const result = {
      kind: "workflowHistory",
      page: { runs: [], nextCursor: null, pendingCount: 0 },
      pageTruncated: false,
      servedAt: Date.now(),
    };
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${job.id}/complete`,
          "POST",
          publisher,
          { result },
        )
      ).status,
    ).toBe(200);
    const read = await f.call(
      `/v1/device/portfolio/queries?key=${input.idempotencyKey}`,
      "GET",
      workflow,
    );
    expect(read.status).toBe(200);
    expect(read.headers.get("Cache-Control")).toBe("no-store");
    const documentInput = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      query: evidence,
    };
    const documentJob = (await (
      await f.call(
        "/v1/device/portfolio/queries",
        "POST",
        documents,
        documentInput,
      )
    ).json()) as { id: string };
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${documentJob.id}/complete`,
          "POST",
          publisher,
          {
            result: {
              kind: "workflowEvidence",
              page: {
                runId: run,
                revision: 1,
                documents: [],
                nextCursor: null,
                total: 0,
              },
              pageTruncated: false,
              servedAt: Date.now(),
            },
          },
        )
      ).status,
    ).toBe(200);
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries?key=${documentInput.idempotencyKey}`,
          "GET",
          workflow,
        )
      ).status,
    ).toBe(200); // no receipt belongs to this phone
    expect(
      (
        (await (
          await f.call(
            `/v1/device/portfolio/queries?key=${documentInput.idempotencyKey}`,
            "GET",
            workflow,
          )
        ).json()) as { job: unknown }
      ).job,
    ).toBeNull();
    expect(
      (await f.call("/v1/device/automations/commands", "POST", documents, {}))
        .status,
    ).toBe(401);
  } finally {
    await f.gateway.close();
  }
});
test("revoked document readers cannot receive a queued result and chunk replies cannot change the selected digest", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      reader = await f.pair(["evidenceRead"]);
    await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    const run = randomUUID(),
      artifact = randomUUID();
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      query: {
        kind: "workflowDocument",
        query: {
          runId: run,
          artifactId: artifact,
          sha256: "a".repeat(64),
          offset: 0,
          limit: 16384,
        },
      },
    };
    const job = (await (
      await f.call("/v1/device/portfolio/queries", "POST", reader, input)
    ).json()) as { id: string };
    const document = {
      id: artifact,
      name: "report.md",
      format: "md",
      projectId: randomUUID(),
      taskId: null,
      stage: 0,
      teamRunId: randomUUID(),
      assignmentId: randomUUID(),
      bytes: 1,
      sha256: "b".repeat(64),
      createdAt: 100,
    };
    const result = {
      kind: "workflowDocument",
      page: { runId: run, document, offset: 0, nextOffset: null, data: "YQ==" },
      pageTruncated: false,
      servedAt: Date.now(),
    };
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries/${job.id}/complete`,
          "POST",
          publisher,
          { result },
        )
      ).status,
    ).toBe(400);
    await f.call(`/v1/owner/devices/${reader.id}/revoke`, "POST", f.owner, {});
    const response = (await (
      await f.call(
        `/v1/device/portfolio/queries/${job.id}/complete`,
        "POST",
        publisher,
        {
          result: {
            ...result,
            page: {
              ...result.page,
              document: { ...document, sha256: "a".repeat(64) },
            },
          },
        },
      )
    ).json()) as { status: string; result: unknown };
    expect(response.status).toBe("rejected");
    expect(response.result).toBeNull();
    expect(
      (
        await f.call(
          `/v1/device/portfolio/queries?key=${input.idempotencyKey}`,
          "GET",
          reader,
        )
      ).status,
    ).toBe(401);
  } finally {
    await f.gateway.close();
  }
});

test("Inbox writes require independent read/control grants, exact publisher and notification-only commands", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      foreign = await f.pair(["publish"]);
    const reader = await f.pair(["inboxRead"]),
      controller = await f.pair(["inboxRead", "inboxControl"]);
    const workflow = await f.pair(["automationRead", "automationControl"]);
    await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      sequence: 1,
      expectedRevision: 0,
      action: "acknowledge",
      expiresAt: Date.now() + 120_000,
    };
    expect(
      (await f.call("/v1/device/inbox/commands", "POST", reader, input)).status,
    ).toBe(401);
    expect(
      (await f.call("/v1/device/inbox/commands", "POST", workflow, input))
        .status,
    ).toBe(401);
    expect(
      (
        await f.call("/v1/device/inbox/commands", "POST", controller, {
          ...input,
          approved: true,
        })
      ).status,
    ).toBe(409);
    const response = await f.call(
      "/v1/device/inbox/commands",
      "POST",
      controller,
      input,
    );
    expect(response.status).toBe(201);
    expect(response.headers.get("cache-control")).toBe("no-store");
    const command = (await response.json()) as any;
    expect(
      (await f.call("/v1/device/inbox/queue", "GET", foreign)).status,
    ).toBe(200);
    expect(
      (
        (await (
          await f.call("/v1/device/inbox/queue", "GET", foreign)
        ).json()) as any
      ).commands,
    ).toEqual([]);
    expect(
      (
        await f.call(
          `/v1/device/inbox/commands/${command.id}/claim`,
          "POST",
          foreign,
        )
      ).status,
    ).not.toBe(200);
    const claim = (await (
      await f.call(
        `/v1/device/inbox/commands/${command.id}/claim`,
        "POST",
        publisher,
      )
    ).json()) as any;
    expect(claim.status).toBe("uncertain");
    expect(
      (
        await f.call(
          `/v1/device/inbox/commands/${command.id}/settle`,
          "POST",
          publisher,
          { applied: true, action: "start" },
        )
      ).status,
    ).toBe(400);
    expect(
      (
        (await (
          await f.call(
            `/v1/device/inbox/commands/${command.id}/settle`,
            "POST",
            publisher,
            { applied: true },
          )
        ).json()) as any
      ).status,
    ).toBe("applied");
    expect(
      (
        (await (
          await f.call(
            `/v1/device/inbox/commands?key=${input.idempotencyKey}`,
            "GET",
            controller,
          )
        ).json()) as any
      ).command.id,
    ).toBe(command.id);
    const another = await f.pair(["inboxRead", "inboxControl"]);
    expect(
      (
        (await (
          await f.call(
            `/v1/device/inbox/commands?key=${input.idempotencyKey}`,
            "GET",
            another,
          )
        ).json()) as any
      ).command,
    ).toBeNull();
  } finally {
    await f.gateway.close();
  }
});
test("revocation rejects queued and uncertain Inbox writes before any owner notification mutation", async () => {
  const f = setup();
  try {
    const publisher = await f.pair(["publish"]),
      controller = await f.pair(["inboxRead", "inboxControl"]);
    await f.call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    const input = {
      publisherId: publisher.id,
      idempotencyKey: randomUUID(),
      sequence: 1,
      expectedRevision: 0,
      action: "archive",
      expiresAt: Date.now() + 120_000,
    };
    const first = (await (
      await f.call("/v1/device/inbox/commands", "POST", controller, input)
    ).json()) as any;
    const second = (await (
      await f.call("/v1/device/inbox/commands", "POST", controller, {
        ...input,
        idempotencyKey: randomUUID(),
      })
    ).json()) as any;
    await f.call(
      `/v1/device/inbox/commands/${second.id}/claim`,
      "POST",
      publisher,
    );
    await f.call(
      `/v1/owner/devices/${controller.id}/revoke`,
      "POST",
      f.owner,
      {},
    );
    for (const command of [first, second]) {
      expect(
        (
          (await (
            await f.call(
              `/v1/device/inbox/commands/${command.id}/claim`,
              "POST",
              publisher,
            )
          ).json()) as any
        ).status,
      ).toBe("rejected");
    }
    expect(
      (
        await f.call(
          `/v1/device/inbox/commands?key=${input.idempotencyKey}`,
          "GET",
          controller,
        )
      ).status,
    ).toBe(401);
  } finally {
    await f.gateway.close();
  }
});
