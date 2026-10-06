import { describe, expect, test } from "bun:test";

import {
  WakuClient,
  WakuConnectionError,
  WakuRpcError,
  daemonUrl,
  type WebSocketLike,
} from "./client";
import { PROTOCOL_VERSION, type ResponsePayload, type ClientMessage } from "./generated";

class FakeSocket implements WebSocketLike {
  readyState = 0;
  sent: string[] = [];
  private listeners = new Map<string, Array<(...args: any[]) => void>>();

  addEventListener(type: "open", listener: () => void): void;
  addEventListener(type: "message", listener: (event: MessageEvent) => void): void;
  addEventListener(type: "error", listener: () => void): void;
  addEventListener(type: "close", listener: (event: CloseEvent) => void): void;
  addEventListener(type: string, listener: (...args: any[]) => void): void {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.readyState = 3;
  }

  emitClose(): void {
    this.readyState = 3;
    this.emit("close", { reason: "" });
  }

  fail(reason: string): void {
    this.readyState = 3;
    this.emit("error", {});
    this.emit("close", { reason });
  }

  open(): void {
    this.readyState = 1;
    this.emit("open");
  }

  receive(message: unknown): void {
    this.emit("message", { data: JSON.stringify(message) });
  }

  private emit(type: string, event?: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

function fixture() {
  const sockets: FakeSocket[] = [];
  let nextId = 0;
  const client = new WakuClient({
    address: "127.0.0.1:4312",
    token: "secret",
    randomUUID: () => `00000000-0000-4000-8000-${String(++nextId).padStart(12, "0")}`,
    webSocketFactory: () => {
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket;
    },
  });
  return { client, sockets };
}

async function connect(client: WakuClient, sockets: FakeSocket[]): Promise<FakeSocket> {
  const connected = client.connect();
  const socket = sockets.at(-1)!;
  socket.open();
  socket.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
  await connected;
  return socket;
}

describe("WakuClient", () => {

  test("sends Teams attachment references and presentation with the original prompt", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);
    const command = {
      type: "queueTeamTask" as const, runId: "00000000-0000-4000-8000-000000000011",
      projectId: "00000000-0000-4000-8000-000000000012", teamId: "00000000-0000-4000-8000-000000000013",
      expectedDefinitionRevision: 8, sourcePrompt: "@Engineering Review attached requirements",
      attachments: [{ path: "C:/daemon/attachments/input/requirements.md", mention: "requirements.md", name: "requirements.md",
        is_dir: false, is_image: false, blob_reference: "waku-attachment:00000000-0000-4000-8000-000000000014" }],
    };
    const pending = client.request(command);
    const sent = JSON.parse(socket.sent.at(-1)!);
    expect(sent.command).toEqual(command);
    socket.receive({ type: "response", requestId: sent.requestId, outcome: { status: "ok", payload: { type: "ack" } } });
    await expect(pending).resolves.toEqual({ type: "ack" });
  });

  test("binds recovery to its attempt and preserves user evidence and the paired snapshot", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);
    const runId = "00000000-0000-4000-8000-000000000011";
    const assignmentId = "00000000-0000-4000-8000-000000000012";
    const attemptId = "00000000-0000-4000-8000-000000000013";
    const agentId = "00000000-0000-4000-8000-000000000014";
    const taskId = "00000000-0000-4000-8000-000000000015";
    const command = {
      type: "confirmTeamRecovery" as const, runId, assignmentId, attemptId, expectedRevision: 7,
      confirmation: { kind: "appFailed" as const, decisionId: "00000000-0000-4000-8000-000000000016", evidence: "Provider rejection verified" },
    };
    const pending = client.request(command);
    const sent = JSON.parse(socket.sent.at(-1)!);
    expect(sent.command).toEqual(command);
    const access = { workspaceRead: true, workspaceWrite: false, shell: false, network: false, appActions: [] };
    const payload = {
      type: "teamRecovery", review: { runId, assignmentId, attemptId,
        revision: 8, host: "local", environment: "local", remoteCalls: [], hostStopped: true, canPrepareContinuation: true, workspacePath: null, files: [], apps: [] },
      snapshot: {
        id: runId, revision: 8, rootTaskId: taskId, projectId: "00000000-0000-4000-8000-000000000017",
        definitionRevision: 1, status: "active", workspace: null, inputs: [], followUps: [], createdAt: 100, deadline: 3700,
        team: { id: "00000000-0000-4000-8000-000000000018", name: "Engineering", objective: "Reviewed outcome", members: [{ agentId, parentId: null }] },
        profiles: [{ id: agentId, name: "Lead", expertise: "Engineering", instructions: "Review evidence", provider: "nebius", model: "fixture-model",
          expectedOutput: "Reviewed document", execution: { environment: "local", sandboxTarget: null, localFallback: true }, access, maxParallelTasks: 1 }],
        limits: { maxWorkItems: 64, maxParallelTasks: 4, maxDepth: 8, maxAttempts: 2, maxModelCalls: 64, maxTokens: 2000000, requireChildReview: true, maxReworkRounds: 2, timeoutSeconds: 3600 },
        usage: { modelCalls: 1, consumedTokens: 0, reservedTokens: 500, unknownCalls: 1, unmeteredCalls: 0, allowanceViolations: 0 }, access,
        work: [{ id: assignmentId, taskId, parentId: null, agentId, objective: "Review provider outcome", expectedOutput: "Reviewed evidence", canResume: false, canReconcile: true,
          outputContract: { minimumDocuments: 1, requiredFormats: [], requiredChecks: [] }, requiresReview: false, reworkOf: null, review: null, dependencies: [], access,
          status: "blocked", reason: "Acknowledgement lost", result: null,
          attempts: [{ id: attemptId, environment: "local", host: "local", fallbackReason: null, startedAt: 100, leaseExpiresAt: 280, generation: 2, status: "blocked" }] }],
        decisions: [],
      },
    } satisfies ResponsePayload;
    socket.receive({ type: "response", requestId: sent.requestId, outcome: { status: "ok", payload } });
    await expect(pending).resolves.toEqual(payload);
    expect(socket.sent).toHaveLength(2);
  });

  test("reports connection state changes, including remote closure", async () => {
    const { client, sockets } = fixture();
    const states: string[] = [];
    const unsubscribe = client.subscribeConnectionState((state) => states.push(state));

    const connected = client.connect();
    const socket = sockets[0]!;
    socket.open();
    socket.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await connected;
    socket.close();
    socket.emitClose();
    unsubscribe();

    expect(states).toEqual(["disconnected", "connecting", "connected", "disconnected"]);
  });

  test("authenticates and correlates typed responses", async () => {
    const { client, sockets } = fixture();
    const connected = client.connect();
    const socket = sockets[0]!;
    socket.open();
    expect(JSON.parse(socket.sent[0]!)).toEqual({
      type: "hello",
      protocolVersion: PROTOCOL_VERSION,
      token: "secret",
      clientId: "00000000-0000-4000-8000-000000000001",
      resumeFrom: [],
    });
    socket.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await connected;

    const response = client.request({ type: "getSettings" });
    const request = JSON.parse(socket.sent[1]!);
    socket.receive({
      type: "response",
      requestId: request.requestId,
      outcome: { status: "ok", payload: { type: "ack" } },
    });
    await expect(response).resolves.toEqual({ type: "ack" });
  });

  test("surfaces daemon errors", async () => {
    const { client, sockets } = fixture();
    const socket = sockets[0] ?? new FakeSocket();
    const connected = client.connect();
    const active = sockets[0] ?? socket;
    active.open();
    active.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await connected;

    const response = client.request({ type: "getSettings" });
    const request = JSON.parse(active.sent[1]!);
    active.receive({
      type: "response",
      requestId: request.requestId,
      outcome: { status: "error", error: { message: "nope" } },
    });
    await expect(response).rejects.toBeInstanceOf(WakuRpcError);
  });

  test("deduplicates events and resumes from the last sequence", async () => {
    const { client, sockets } = fixture();
    const firstConnection = client.connect();
    const first = sockets[0]!;
    first.open();
    first.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await firstConnection;

    const received: number[] = [];
    client.subscribe("session", "runtime", (event) => received.push(event.sequence));
    const event = {
      type: "event",
      sessionId: "session",
      runtimeId: "runtime",
      epoch: "epoch-one",
      sequence: 4,
      event: { kind: "textDelta", payload: { text: "hi" } },
    };
    first.receive(event);
    first.receive(event);
    expect(received).toEqual([4]);

    client.disconnect();
    const secondConnection = client.connect();
    const second = sockets[1]!;
    second.open();
    expect(JSON.parse(second.sent[0]!).resumeFrom).toEqual([
      { sessionId: "session", runtimeId: "runtime", epoch: "epoch-one", sequence: 4 },
    ]);
    second.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await secondConnection;
  });

  test("disconnect rejects an in-flight handshake and permits reconnecting", async () => {
    const { client, sockets } = fixture();
    const firstConnection = client.connect();
    client.disconnect();
    await expect(firstConnection).rejects.toThrow("Waku client disconnected");

    const secondConnection = client.connect();
    const second = sockets[1]!;
    second.open();
    second.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await expect(secondConnection).resolves.toBeUndefined();
  });

  test("surfaces the native socket failure reason", async () => {
    const { client, sockets } = fixture();
    const connection = client.connect();

    sockets[0]!.fail("The network connection was lost");

    await expect(connection).rejects.toThrow("The network connection was lost");
  });

  test("accepts sequence one again when the daemon epoch changes", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);
    const received: Array<[string, number]> = [];
    client.subscribe("session", "runtime", (event) => {
      received.push([event.epoch, event.sequence]);
    });

    socket.receive({
      type: "event",
      sessionId: "session",
      runtimeId: "runtime",
      epoch: "old",
      sequence: 9,
      event: { kind: "textDelta", payload: null },
    });
    socket.receive({
      type: "event",
      sessionId: "session",
      runtimeId: "runtime",
      epoch: "new",
      sequence: 1,
      event: { kind: "textDelta", payload: null },
    });

    expect(received).toEqual([
      ["old", 9],
      ["new", 1],
    ]);
  });

  test("buffers replayed events until a refreshed app attaches to the runtime", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);

    socket.receive({
      type: "event",
      sessionId: "session",
      runtimeId: "runtime",
      epoch: "epoch",
      sequence: 1,
      event: { kind: "textDelta", payload: "before attach" },
    });
    socket.receive({
      type: "event",
      sessionId: "session",
      runtimeId: "runtime",
      epoch: "epoch",
      sequence: 2,
      event: { kind: "textDelta", payload: "still before attach" },
    });

    const received: number[] = [];
    client.subscribe("session", "runtime", (event) => received.push(event.sequence));
    expect(received).toEqual([1, 2]);
  });

  test("notifies connected apps when another client changes task state", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);
    const revisions: number[] = [];
    client.subscribeTaskState((revision) => revisions.push(revision));

    socket.receive({ type: "taskStateChanged", revision: 7 });
    expect(revisions).toEqual([7]);
  });

  test("disconnected requests reject instead of throwing synchronously", async () => {
    const { client } = fixture();
    const request = client.request({ type: "getSettings" });
    await expect(request).rejects.toThrow("Waku daemon is disconnected");
  });

  test("disconnected notifications reject instead of throwing synchronously", async () => {
    const { client } = fixture();
    const notification = client.notify({ type: "refreshBackgroundWork" });
    await expect(notification).rejects.toThrow("Waku daemon is disconnected");
  });

  test("notifications use the response-free nil request id", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);

    await client.notify(
      { type: "writeTerminal", data: "bHM=" },
      "terminal",
      "terminal",
    );

    expect(JSON.parse(socket.sent[1]!)).toEqual({
      type: "request",
      requestId: "00000000-0000-0000-0000-000000000000",
      sessionId: "terminal",
      runtimeId: "terminal",
      command: { type: "writeTerminal", data: "bHM=" },
    });
  });
});

test("daemonUrl pins the versioned endpoint", () => {
  expect(daemonUrl("localhost:3030/anything?old=1")).toBe("ws://localhost:3030/v1");
  expect(daemonUrl("wss://waku.example.test")).toBe("wss://waku.example.test/v1");
});

describe("WakuClient connection failures", () => {
  async function failure(setup: (socket: FakeSocket) => void): Promise<WakuConnectionError> {
    const { client, sockets } = fixture();
    const connected = client.connect();
    setup(sockets[0]!);
    const error = await connected.catch((cause: unknown) => cause);
    expect(error).toBeInstanceOf(WakuConnectionError);
    return error as WakuConnectionError;
  }

  test("types handshake failures so callers can tell what needs a person", async () => {
    const rejected = await failure((socket) => {
      socket.open();
      socket.receive({ type: "rejected", message: "authentication failed" });
    });
    expect(rejected.kind).toBe("rejected");
    expect(rejected.retryable).toBe(false);
    expect(rejected.message).toBe("daemon rejected connection: authentication failed");

    const unsupported = await failure((socket) => {
      socket.open();
      socket.receive({ type: "rejected", message: "protocol 3 is unsupported; expected 7" });
    });
    expect(unsupported.kind).toBe("protocol");

    const mismatch = await failure((socket) => {
      socket.open();
      socket.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION + 1, daemonVersion: "x" });
    });
    expect(mismatch.kind).toBe("protocol");
    expect(mismatch.retryable).toBe(false);

    const garbage = await failure((socket) => {
      socket.open();
      socket.receive({ type: "event", sessionId: "x" });
    });
    expect(garbage.kind).toBe("handshake");

    const refused = await failure((socket) => socket.fail("Connection refused"));
    expect(refused.kind).toBe("unreachable");
    expect(refused.retryable).toBe(true);
    expect(refused.message).toBe("Connection refused");
  });

  test("times out a handshake the daemon never answers", async () => {
    const sockets: FakeSocket[] = [];
    const client = new WakuClient({
      address: "127.0.0.1:4312",
      token: "secret",
      connectTimeoutMs: 1,
      randomUUID: () => "00000000-0000-4000-8000-000000000001",
      webSocketFactory: () => {
        const socket = new FakeSocket();
        sockets.push(socket);
        return socket;
      },
    });
    const error = (await client.connect().catch((cause: unknown) => cause)) as WakuConnectionError;
    expect(error.kind).toBe("timeout");
    expect(error.retryable).toBe(true);
    expect(client.connectionState).toBe("disconnected");
    expect(sockets[0]!.readyState).toBe(3);

    // The abandoned socket's late events are ignored and a fresh attempt works.
    sockets[0]!.open();
    const again = client.connect();
    expect(sockets).toHaveLength(2);
    sockets[1]!.open();
    sockets[1]!.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await again;
    expect(client.connected).toBe(true);
  });

  test("lets a request shorten its own timeout", async () => {
    const { client, sockets } = fixture();
    await connect(client, sockets);
    await expect(
      client.request({ type: "getSettings" }, undefined, undefined, { timeoutMs: 1 }),
    ).rejects.toThrow("timed out waiting for Waku daemon");
  });

  test("settles requests and clears the socket before listeners hear a remote close", async () => {
    const { client, sockets } = fixture();
    const socket = await connect(client, sockets);
    const request = client.request({ type: "getSettings" });
    let reconnected: Promise<void> | null = null;
    client.subscribeConnectionState((state) => {
      if (state === "disconnected" && !reconnected) reconnected = client.connect();
    });

    socket.fail("Software caused connection abort");
    const error = (await request.catch((cause: unknown) => cause)) as WakuConnectionError;
    expect(error.kind).toBe("closed");
    expect(client.lastDisconnectReason).toBe("Software caused connection abort");
    expect(sockets).toHaveLength(2);

    sockets[1]!.open();
    sockets[1]!.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await reconnected!;
    expect(client.connected).toBe(true);
    const next = client.request({ type: "getSettings" });
    expect(sockets[1]!.sent).toHaveLength(2);
    const sent = JSON.parse(sockets[1]!.sent[1]!) as { requestId: string };
    sockets[1]!.receive({
      type: "response",
      requestId: sent.requestId,
      outcome: { status: "ok", payload: { type: "ack" } },
    });
    await expect(next).resolves.toEqual({ type: "ack" });
  });

  test("records when the daemon last spoke", async () => {
    const sockets: FakeSocket[] = [];
    let now = 41;
    const client = new WakuClient({
      address: "127.0.0.1:4312",
      token: "secret",
      now: () => now,
      randomUUID: () => "00000000-0000-4000-8000-000000000001",
      webSocketFactory: () => {
        const socket = new FakeSocket();
        sockets.push(socket);
        return socket;
      },
    });
    expect(client.lastMessageAt).toBe(0);
    const connected = client.connect();
    now = 42;
    sockets[0]!.open();
    sockets[0]!.receive({ type: "hello", protocolVersion: PROTOCOL_VERSION, daemonVersion: "test" });
    await connected;
    expect(client.lastMessageAt).toBe(42);
    now = 43;
    sockets[0]!.receive({ type: "taskStateChanged", revision: 1 });
    expect(client.lastMessageAt).toBe(43);
  });
});


test("Queues a team follow-up with stable message identity and the task route", async () => {
  const { client, sockets } = fixture(); const socket = await connect(client, sockets)
  const message = { id: 'followup', content: 'Include API evidence', display_content: null, attachments: [], created_at: 180 }
  const pending = client.request({ type: 'queueTeamFollowUp', runId: 'run', message }, 'lead-task')
  const request = JSON.parse(socket.sent.at(-1)!)
  expect(request.sessionId).toBe('lead-task')
  expect(request.command).toEqual({ type: 'queueTeamFollowUp', runId: 'run', message })
  socket.receive({ type: 'response', requestId: request.requestId, outcome: { status: 'ok', payload: { type: 'ack' } } })
  await expect(pending).resolves.toEqual({ type: 'ack' })
})


test('automation requests preserve start identity, pinned revision and gate scope', async () => {
  const { client, sockets } = fixture(); const socket = await connect(client, sockets);
  const command = { type: 'startAutomation' as const, id: 'automation', definitionId: 'workflow', expectedDefinitionRevision: 42 };
  const first = client.request(command); const request = JSON.parse(socket.sent.at(-1)!) as Extract<ClientMessage, { type: 'request' }>;
  expect(request.command).toEqual(command);
  socket.receive({ type: 'response', requestId: request.requestId, outcome: { status: 'ok', payload: { type: 'ack' } } }); await first;
  const next = client.request({ type: 'resolveAutomationGate', id: 'automation', expectedRevision: 5, stage: 2, approved: false });
  const gate = JSON.parse(socket.sent.at(-1)!) as Extract<ClientMessage, { type: 'request' }>;
  expect(gate.command).toEqual({ type: 'resolveAutomationGate', id: 'automation', expectedRevision: 5, stage: 2, approved: false });
  socket.receive({ type: 'response', requestId: gate.requestId, outcome: { status: 'ok', payload: { type: 'ack' } } }); await next;
});
