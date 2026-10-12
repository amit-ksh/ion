import type { PushTopic } from "@waku/client";
import {
  validateInboxCommand,
  type InboxCommand,
  type InboxSubmission,
} from "@waku/client";
import {
  validatePortfolioJob,
  portfolioId,
  type PortfolioSubmission,
  type PortfolioJob,
  type PortfolioOwner,
} from "@waku/client";
import type {
  AutomationCommand,
  AutomationOwner,
  AutomationSubmission,
} from "@waku/client";
export interface GatewayPairing {
  baseUrl: string;
  id: string;
  token: string;
  expiresAt: number;
  grants: string[];
}

export interface GatewayProposal {
  id: string;
  deviceId: string;
  fingerprint: string;
  status: "pending" | "approved" | "denied" | "uncertain" | "succeeded";
  effect: {
    toolkit: string;
    tool: string;
    version: string;
    accountId: string;
    target: string;
    body: string;
    scopes: string[];
  };
  reviewedBy?: string;
}

export interface GatewayTask {
  id: string;
  projectId: string;
  projectName: string;
  title: string;
  provider: string;
  status: string;
  runtimeId: string | null;
  updatedAt: number;
  publisherSeenAt?: number | null;
  computerTarget?: {
    taskId: string;
    runtimeId: string;
    appName: string;
    windowTitle: string;
    phase: "awaitingApproval" | "running" | "failed";
    visible: boolean;
  } | null;
}

export interface GatewayTaskEvent {
  cursor: number;
  taskId: string;
  kind: "message" | "activity" | "artifact" | "diff" | "approval" | "status";
  text: string;
  at: number;
}

export interface GatewayTaskCommand {
  id: string;
  taskId: string;
  runtimeId: string;
  kind: "stop" | "steer" | "permission" | "answer";
  status: "queued" | "uncertain" | "applied" | "rejected";
  createdAt: number;
  updatedAt: number;
}

export interface GatewayTaskInteraction {
  id: string;
  taskId: string;
  runtimeId: string;
  requestId: string;
  kind: "permission" | "question";
  title: string;
  detail: string;
  options: Array<{ id: string; label: string; allow: boolean }>;
  questions: Array<{
    id: string;
    header: string;
    question: string;
    options: Array<{ label: string; description?: string }>;
    multiSelect: boolean;
  }>;
  fingerprint: string;
  status: "pending" | "resolved";
  publishedAt: number;
}

export function gatewayBaseUrl(input: string): string {
  let url: URL;
  try {
    url = new URL(input.trim());
  } catch {
    throw new Error("Enter a valid gateway URL");
  }
  const loopback = ["127.0.0.1", "localhost", "[::1]"].includes(
    url.hostname.toLowerCase(),
  );
  if (url.protocol !== "https:" && !(url.protocol === "http:" && loopback)) {
    throw new Error("Remote gateways require HTTPS");
  }
  if (
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.search ||
    url.hash
  ) {
    throw new Error(
      "Enter only the gateway origin, without credentials or a path",
    );
  }
  return url.origin;
}

export class GatewayRequestError extends Error {
  constructor(readonly status: number) {
    super(`Gateway request failed (HTTP ${status})`);
  }
}

async function request<T>(
  baseUrl: string,
  path: string,
  init: RequestInit,
): Promise<T> {
  const abort = new AbortController();
  const timeout = setTimeout(() => abort.abort(), 20_000);
  try {
    const response = await fetch(`${baseUrl}${path}`, {
      ...init,
      signal: abort.signal,
      cache: "no-store",
      redirect: "error",
    });
    if (!response.ok) throw new GatewayRequestError(response.status);
    return (await response.json()) as T;
  } finally {
    clearTimeout(timeout);
  }
}

function deviceHeaders(pairing: GatewayPairing): HeadersInit {
  return {
    Authorization: `Bearer ${pairing.token}`,
    "X-Waku-Device-Id": pairing.id,
  };
}

export async function pairGateway(
  baseUrlInput: string,
  invite: string,
): Promise<GatewayPairing> {
  const baseUrl = gatewayBaseUrl(baseUrlInput);
  if (!invite.trim()) throw new Error("Enter the one-use invite");
  const result = await request<Omit<GatewayPairing, "baseUrl">>(
    baseUrl,
    "/v1/pair",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ invite: invite.trim(), name: "ion mobile" }),
    },
  );
  if (
    !/^[0-9a-f-]{36}$/.test(result.id) ||
    !result.token ||
    !Number.isFinite(result.expiresAt) ||
    !Array.isArray(result.grants)
  ) {
    throw new Error("Gateway returned an invalid pairing");
  }
  return { ...result, baseUrl };
}

export async function gatewayReviewQueue(
  pairing: GatewayPairing,
): Promise<GatewayProposal[]> {
  const response = await request<{ proposals: GatewayProposal[] }>(
    pairing.baseUrl,
    "/v1/device/review-queue",
    { headers: deviceHeaders(pairing) },
  );
  if (!Array.isArray(response.proposals))
    throw new Error("Gateway returned an invalid review queue");
  return response.proposals;
}

export async function decideGatewayProposal(
  pairing: GatewayPairing,
  proposal: GatewayProposal,
  approve: boolean,
): Promise<GatewayProposal> {
  return request<GatewayProposal>(
    pairing.baseUrl,
    `/v1/device/proposals/${proposal.id}/decision`,
    {
      method: "POST",
      headers: {
        ...deviceHeaders(pairing),
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ fingerprint: proposal.fingerprint, approve }),
    },
  );
}

export async function gatewayReviewStatus(
  pairing: GatewayPairing,
  proposalId: string,
): Promise<GatewayProposal> {
  return request<GatewayProposal>(
    pairing.baseUrl,
    `/v1/device/reviews/${proposalId}`,
    { headers: deviceHeaders(pairing) },
  );
}

export async function gatewayOwnerTask(
  pairing: GatewayPairing,
  publisherId: string,
  taskId: string,
): Promise<GatewayTask> {
  if (
    !portfolioId(publisherId) ||
    !portfolioId(taskId) ||
    !pairing.grants.includes("taskRead") ||
    pairing.expiresAt <= Date.now()
  )
    throw new Error("Pair with Read tasks to open the original owner task");
  const reply = await request<unknown>(
    pairing.baseUrl,
    `/v1/device/tasks/${taskId}?publisherId=${publisherId}`,
    { headers: deviceHeaders(pairing) },
  );
  const object = (v: unknown): v is Record<string, any> =>
    !!v && typeof v === "object" && !Array.isArray(v);
  if (
    !object(reply) ||
    Object.keys(reply).sort().join(",") !== "publisherId,task" ||
    reply.publisherId !== publisherId ||
    !object(reply.task)
  )
    throw new Error("Task response crossed owner scope");
  const task = reply.task;
  if (
    Object.keys(task).sort().join(",") !==
      "id,projectId,projectName,provider,publisherSeenAt,runtimeId,status,title,updatedAt" ||
    task.id !== taskId ||
    !portfolioId(task.projectId) ||
    (task.runtimeId !== null && !portfolioId(task.runtimeId)) ||
    !["projectName", "provider", "status", "title"].every(
      (k) => typeof task[k] === "string" && task[k].length <= 1000,
    ) ||
    !Number.isSafeInteger(task.updatedAt) ||
    task.updatedAt < 0 ||
    (task.publisherSeenAt !== null &&
      (!Number.isSafeInteger(task.publisherSeenAt) || task.publisherSeenAt < 0))
  )
    throw new Error("Invalid original task snapshot");
  return task as GatewayTask;
}

export async function gatewayTasks(pairing: GatewayPairing): Promise<{
  tasks: GatewayTask[];
  publishedAt: number | null;
}> {
  return request(pairing.baseUrl, "/v1/device/tasks", {
    headers: deviceHeaders(pairing),
  });
}

function pushPath(topic: PushTopic): string {
  if (topic !== "taskRequests" && topic !== "inbox")
    throw new Error("Invalid notification topic");
  return topic === "inbox" ? "/v1/device/inbox/push" : "/v1/device/push";
}
export async function gatewayPushStatus(
  pairing: GatewayPairing,
  topic: PushTopic = "taskRequests",
): Promise<boolean> {
  const result = await request<{ enabled: boolean }>(
    pairing.baseUrl,
    pushPath(topic),
    { headers: deviceHeaders(pairing) },
  );
  return result.enabled === true;
}

export async function registerGatewayPush(
  pairing: GatewayPairing,
  token: string,
  topic: PushTopic = "taskRequests",
): Promise<void> {
  await request(pairing.baseUrl, pushPath(topic), {
    method: "PUT",
    headers: { ...deviceHeaders(pairing), "Content-Type": "application/json" },
    body: JSON.stringify({ token }),
  });
}

export async function unregisterGatewayPush(
  pairing: GatewayPairing,
  topic: PushTopic = "taskRequests",
): Promise<void> {
  await request(pairing.baseUrl, pushPath(topic), {
    method: "DELETE",
    headers: deviceHeaders(pairing),
  });
}

export async function gatewayTaskEvents(
  pairing: GatewayPairing,
  after: number,
): Promise<{
  events: GatewayTaskEvent[];
  nextCursor: number;
  resetRequired: boolean;
}> {
  if (!Number.isSafeInteger(after) || after < 0)
    throw new Error("Invalid event cursor");
  return request(pairing.baseUrl, `/v1/device/events?after=${after}`, {
    headers: deviceHeaders(pairing),
  });
}

export async function gatewayTaskInteractions(
  pairing: GatewayPairing,
  taskId: string,
): Promise<GatewayTaskInteraction[]> {
  if (!/^[0-9a-f-]{36}$/.test(taskId)) throw new Error("Invalid task ID");
  const response = await request<{ interactions: GatewayTaskInteraction[] }>(
    pairing.baseUrl,
    `/v1/device/tasks/${taskId}/interactions`,
    { headers: deviceHeaders(pairing) },
  );
  if (!Array.isArray(response.interactions))
    throw new Error("Gateway returned invalid interactions");
  return response.interactions;
}

export async function enqueueGatewayTaskCommand(
  pairing: GatewayPairing,
  input: {
    idempotencyKey: string;
    taskId: string;
    runtimeId: string;
    kind: "stop" | "steer" | "permission" | "answer";
    payload: Record<string, string>;
  },
  publisherId?: string,
): Promise<GatewayTaskCommand> {
  if (
    publisherId !== undefined &&
    (!portfolioId(publisherId) ||
      !portfolioId(input.taskId) ||
      !portfolioId(input.runtimeId) ||
      !portfolioId(input.idempotencyKey) ||
      !pairing.grants.includes("taskControl") ||
      pairing.expiresAt <= Date.now())
  )
    throw new Error("Invalid command owner or task control grant");
  const reply = await request<
    GatewayTaskCommand & { publisherId?: string; deviceId?: string }
  >(
    pairing.baseUrl,
    publisherId === undefined
      ? "/v1/device/commands"
      : `/v1/device/commands?publisherId=${publisherId}`,
    {
      method: "POST",
      headers: {
        ...deviceHeaders(pairing),
        "Content-Type": "application/json",
      },
      body: JSON.stringify(input),
    },
  );
  if (
    publisherId !== undefined &&
    (reply.publisherId !== publisherId ||
      reply.deviceId !== pairing.id ||
      reply.taskId !== input.taskId ||
      reply.runtimeId !== input.runtimeId ||
      reply.kind !== input.kind ||
      !portfolioId(reply.id) ||
      !["queued", "uncertain", "applied", "rejected"].includes(reply.status))
  )
    throw new Error("Command receipt crossed owner or task scope");
  return reply;
}

export async function gatewayTaskCommand(
  pairing: GatewayPairing,
  id: string,
  publisherId?: string,
): Promise<GatewayTaskCommand> {
  if (
    !portfolioId(id) ||
    (publisherId !== undefined && !portfolioId(publisherId))
  )
    throw new Error("Invalid task command ID or owner");
  const reply = await request<
    GatewayTaskCommand & { publisherId?: string; deviceId?: string }
  >(pairing.baseUrl, `/v1/device/commands/${id}`, {
    headers: deviceHeaders(pairing),
  });
  if (
    publisherId !== undefined &&
    (reply.publisherId !== publisherId ||
      reply.deviceId !== pairing.id ||
      reply.id !== id ||
      !portfolioId(reply.taskId) ||
      !portfolioId(reply.runtimeId) ||
      !["stop", "steer", "permission", "answer"].includes(reply.kind) ||
      !["queued", "uncertain", "applied", "rejected"].includes(reply.status))
  )
    throw new Error("Command receipt crossed owner scope");
  return reply;
}

const automationUuid =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const automationHash = /^[a-f0-9]{64}$/;
function automationCounter(
  value: unknown,
  max = Number.MAX_SAFE_INTEGER,
): value is number {
  return (
    typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= 0 &&
    value <= max
  );
}
function automationLabel(value: unknown, max: number): value is string {
  return typeof value === "string" && value.length <= max;
}
function validAutomationOwner(
  owner: AutomationOwner & { name: string },
): boolean {
  if (
    !owner ||
    !automationUuid.test(owner.publisherId) ||
    owner.version !== 1 ||
    !automationCounter(owner.revision) ||
    !automationCounter(owner.publishedAt) ||
    typeof owner.truncated !== "boolean" ||
    !automationLabel(owner.name, 240) ||
    !Array.isArray(owner.projects) ||
    owner.projects.length > 64 ||
    !Array.isArray(owner.definitions) ||
    owner.definitions.length > 64 ||
    !Array.isArray(owner.runs) ||
    owner.runs.length > 64
  )
    return false;
  const projects = new Set<string>();
  const definitions = new Set<string>();
  const runs = new Set<string>();
  for (const p of owner.projects) {
    if (
      !p ||
      !automationUuid.test(p.id) ||
      projects.has(p.id) ||
      !automationLabel(p.name, 160)
    )
      return false;
    projects.add(p.id);
  }
  for (const d of owner.definitions) {
    if (
      !d ||
      !automationUuid.test(d.id) ||
      definitions.has(d.id) ||
      !projects.has(d.projectId) ||
      !automationHash.test(d.fingerprint) ||
      !automationLabel(d.name, 160) ||
      !["workflow", "routine"].includes(d.kind) ||
      d.revision !== owner.revision ||
      typeof d.ready !== "boolean" ||
      !automationCounter(d.stageCount, 24) ||
      (d.kind === "workflow" &&
        (d.paused !== null || d.nextRun !== null || d.schedule !== null)) ||
      (d.kind === "routine" &&
        (typeof d.paused !== "boolean" ||
          !automationCounter(d.nextRun) ||
          !automationLabel(d.schedule, 500)))
    )
      return false;
    definitions.add(d.id);
  }
  for (const r of owner.runs) {
    if (
      !r ||
      !automationUuid.test(r.id) ||
      runs.has(r.id) ||
      !projects.has(r.projectId) ||
      !automationUuid.test(r.definitionId) ||
      !automationHash.test(r.fingerprint) ||
      !automationLabel(r.name, 160) ||
      !automationCounter(r.revision) ||
      ![
        "running",
        "waitingDecision",
        "blocked",
        "completed",
        "canceled",
        "skipped",
      ].includes(r.status) ||
      !automationCounter(r.stageCount, 24) ||
      r.stageCount === 0 ||
      !automationCounter(r.currentStage, r.stageCount)
    )
      return false;
    if (
      r.gate !== null &&
      (!r.gate ||
        r.status !== "waitingDecision" ||
        !automationCounter(r.gate.index, r.stageCount - 1) ||
        r.gate.index !== r.currentStage ||
        !automationLabel(r.gate.title, 160) ||
        !automationLabel(r.gate.instructions, 2000) ||
        typeof r.gate.reviewComplete !== "boolean")
    )
      return false;
    if (r.status === "waitingDecision" && r.gate === null) return false;
    runs.add(r.id);
  }
  return true;
}
export async function gatewayAutomationOwners(
  pairing: GatewayPairing,
): Promise<(AutomationOwner & { name: string })[]> {
  const feed = await request<{
    owners: (AutomationOwner & { name: string })[];
  }>(pairing.baseUrl, "/v1/device/automations", {
    headers: deviceHeaders(pairing),
  });
  if (
    !Array.isArray(feed.owners) ||
    feed.owners.length > 64 ||
    feed.owners.some((owner) => !validAutomationOwner(owner)) ||
    new Set(feed.owners.map((o) => o.publisherId)).size !== feed.owners.length
  )
    throw new Error("Invalid workflow feed");
  return feed.owners;
}
export async function gatewayEnqueueAutomation(
  pairing: GatewayPairing,
  input: AutomationSubmission,
): Promise<AutomationCommand> {
  return request(pairing.baseUrl, "/v1/device/automations/commands", {
    method: "POST",
    headers: { ...deviceHeaders(pairing), "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}
export async function gatewayAutomationCommandByKey(
  pairing: GatewayPairing,
  key: string,
): Promise<AutomationCommand | null> {
  const result = await request<{ command: AutomationCommand | null }>(
    pairing.baseUrl,
    `/v1/device/automations/commands?key=${encodeURIComponent(key)}`,
    { headers: deviceHeaders(pairing) },
  );
  if (!Object.hasOwn(result, "command"))
    throw new Error("Invalid workflow command lookup");
  return result.command;
}
export async function gatewayAutomationCommands(
  pairing: GatewayPairing,
): Promise<AutomationCommand[]> {
  const result = await request<{ commands: AutomationCommand[] }>(
    pairing.baseUrl,
    "/v1/device/automations/commands",
    { headers: deviceHeaders(pairing) },
  );
  if (
    !Array.isArray(result.commands) ||
    result.commands.length > 32 ||
    result.commands.some(
      (command) =>
        !command ||
        command.deviceId !== pairing.id ||
        !["queued", "uncertain", "applied", "rejected"].includes(
          command.status,
        ),
    )
  )
    throw new Error("Invalid workflow command history");
  return result.commands;
}

export async function gatewayPortfolioOwners(
  pairing: GatewayPairing,
): Promise<PortfolioOwner[]> {
  const result = await request<{ owners: PortfolioOwner[] }>(
    pairing.baseUrl,
    "/v1/device/portfolio",
    { method: "GET", headers: deviceHeaders(pairing) },
  );
  if (
    !Array.isArray(result.owners) ||
    result.owners.length > 64 ||
    result.owners.some(
      (owner) =>
        !portfolioId(owner.publisherId) ||
        typeof owner.name !== "string" ||
        owner.name.length > 160 ||
        !Number.isSafeInteger(owner.publishedAt) ||
        owner.publishedAt < 0,
    ) ||
    new Set(result.owners.map((owner) => owner.publisherId)).size !==
      result.owners.length
  )
    throw new Error("Invalid registered owner directory");
  return result.owners;
}
export async function submitGatewayPortfolio(
  pairing: GatewayPairing,
  input: PortfolioSubmission,
): Promise<PortfolioJob> {
  const result: unknown = await request(
    pairing.baseUrl,
    "/v1/device/portfolio/queries",
    {
      method: "POST",
      headers: {
        ...deviceHeaders(pairing),
        "Content-Type": "application/json",
      },
      body: JSON.stringify(input),
    },
  );
  validatePortfolioJob(result, pairing.id, input);
  return result;
}
export async function readGatewayPortfolio(
  pairing: GatewayPairing,
  input: PortfolioSubmission,
): Promise<PortfolioJob | null> {
  const result = await request<{ job: unknown }>(
    pairing.baseUrl,
    `/v1/device/portfolio/queries?key=${encodeURIComponent(input.idempotencyKey)}`,
    { method: "GET", headers: deviceHeaders(pairing) },
  );
  if (result.job === null) return null;
  validatePortfolioJob(result.job, pairing.id, input);
  return result.job;
}

export async function gatewayEnqueueInbox(
  pairing: GatewayPairing,
  input: InboxSubmission,
): Promise<InboxCommand> {
  const result = await request<unknown>(
    pairing.baseUrl,
    "/v1/device/inbox/commands",
    {
      method: "POST",
      headers: {
        ...deviceHeaders(pairing),
        "Content-Type": "application/json",
      },
      body: JSON.stringify(input),
    },
  );
  validateInboxCommand(result);
  return result;
}
export async function gatewayInboxByKey(
  pairing: GatewayPairing,
  key: string,
): Promise<InboxCommand | null> {
  const result = await request<{ command: unknown }>(
    pairing.baseUrl,
    `/v1/device/inbox/commands?key=${encodeURIComponent(key)}`,
    { headers: deviceHeaders(pairing) },
  );
  if (!result || Object.keys(result).join(",") !== "command")
    throw new Error("Invalid Inbox command lookup");
  if (result.command === null) return null;
  validateInboxCommand(result.command);
  return result.command;
}
