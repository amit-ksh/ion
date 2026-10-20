/** Operator-only pull controller. Do not package with user apps or install on
 * the controlled VM. It owns no public listener and never accepts arbitrary IDs. */
import { SDK } from "@nebius/js-sdk";
import { CredentialsFileReader } from "@nebius/js-sdk/runtime/service_account/credentials_file";
import { InstanceService, GetInstanceRequest, StartInstanceRequest, StopInstanceRequest } from "@nebius/js-sdk/api/nebius/compute/v1/index";
import { GetOperationRequest } from "@nebius/js-sdk/api/nebius/common/v1/index";
import { Metadata } from "@grpc/grpc-js";
import { mkdir, readFile, writeFile, rename, lstat, unlink } from "node:fs/promises";
import { join, isAbsolute } from "node:path";
import { randomBytes } from "node:crypto";
const origin = process.env.ION_CLOUD_PUBLIC_ORIGIN ?? "",
  token = process.env.ION_COMPUTE_CONTROLLER_TOKEN ?? "",
  instanceId = process.env.ION_COMPUTE_INSTANCE_ID ?? "",
  projectId = "project-e04qppz9bc0026zt9y9ky7",
  credentialFile = process.env.ION_NEBIUS_CREDENTIALS_FILE ?? "",
  directory = process.env.ION_COMPUTE_STATE_DIRECTORY ?? "";
const url = new URL(origin);
if (url.protocol !== "https:" || origin !== url.origin || url.username || url.password ||
    !/^[A-Za-z0-9_-]{43,128}$/.test(token) || !/^computeinstance-[a-z0-9]+$/.test(instanceId) ||
    !isAbsolute(credentialFile) || !isAbsolute(directory))
  throw new Error("Configure the operator controller origin, exact instance, credentials file and private state directory");
process.umask(0o077);
async function regular(path: string, maximum: number) {
  const info = await lstat(path);
  if (!info.isFile() || info.isSymbolicLink() || info.size > maximum || (process.platform !== "win32" && (info.mode & 0o077)))
    throw new Error("Controller files must be private regular files");
}
await regular(credentialFile, 32768);
await mkdir(directory, { recursive: true, mode: 0o700 });
const dirInfo = await lstat(directory);
if (!dirInfo.isDirectory() || dirInfo.isSymbolicLink() || (process.platform !== "win32" && (dirInfo.mode & 0o077)))
  throw new Error("Controller state must be private");
const lock = join(directory, "controller.lock"), file = join(directory, "controller.json");
try {
  await regular(lock, 100);
  const pid = Number(await readFile(lock, "utf8"));
  if (!Number.isSafeInteger(pid) || pid < 1) throw new Error("Invalid controller lock");
  let alive = true;
  try { process.kill(pid, 0); } catch (e) { if ((e as NodeJS.ErrnoException).code === "ESRCH") alive = false; }
  if (alive) throw new Error("Another controller is running; do not share its state directory");
  await unlink(lock);
} catch (e) { if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e; }
await writeFile(lock, String(process.pid), { flag: "wx", mode: 0o600 });
type State = { version: 1; instanceId: string; controllerId: string; intent?: "start" | "stop"; intentAt?: number; operation?: string; stopAt?: number };
let state: State;
try {
  await regular(file, 4000); state = JSON.parse(await readFile(file, "utf8"));
  if (state.version !== 1 || state.instanceId !== instanceId || !/^[A-Za-z0-9_-]{24,80}$/.test(state.controllerId) ||
      (state.operation && !/^[A-Za-z0-9_-]{1,200}$/.test(state.operation))) throw new Error("Controller state needs operator repair");
} catch (e) {
  if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
  state = { version: 1, instanceId, controllerId: randomBytes(24).toString("base64url") };
}
async function save() {
  const partial = file + ".partial";
  await writeFile(partial, JSON.stringify(state), { mode: 0o600 }); await rename(partial, file);
}
await save();
const sdk = new SDK({ credentials: new CredentialsFileReader(credentialFile), userAgentPrefix: "ion-compute-controller/1.0", parentId: projectId });
const instances = new InstanceService(sdk), operations = instances.getOperationService();
const options = () => ({ deadline: new Date(Date.now() + 15_000), RequestTimeout: 12_000, PerRetryTimeout: 12_000, RetryCount: 0 });
let stopping = false, lastGateway = 0;
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => { stopping = true; });
async function mutate(action: "start" | "stop") {
  state.intent = action; state.intentAt = Date.now(); delete state.operation; await save();
  const operation = action === "start"
    ? await instances.start(StartInstanceRequest.create({ id: instanceId }), new Metadata(), options()).result
    : await instances.stop(StopInstanceRequest.create({ id: instanceId }), new Metadata(), options()).result;
  state.operation = operation.id(); await save();
}
try {
  while (!stopping) {
    try {
      let operationSettled = !state.intent;
      if (state.operation) {
        const op = await operations.get(GetOperationRequest.create({ id: state.operation }), new Metadata(), options()).result;
        if (op.done()) { delete state.operation; delete state.intent; await save(); operationSettled = true; }
      }
      const instance = await instances.get(GetInstanceRequest.create({ id: instanceId }), new Metadata(), options());
      if (instance.metadata?.id !== instanceId || instance.metadata.parentId !== projectId)
        throw new Error("The configured VM belongs to another project");
      const providerState = instance.status?.state.name ?? "UNKNOWN";
      const observed = ["RUNNING", "STOPPED", "STARTING", "STOPPING"].includes(providerState) ? providerState : "UNKNOWN";
      const response = await fetch(origin + "/internal/compute/tick", {
        method: "POST", headers: { Authorization: "Bearer " + token, "Content-Type": "application/json" },
        body: JSON.stringify({ instanceId, projectId, controllerId: state.controllerId, state: observed,
          operationSettled: operationSettled && !instance.status?.reconciling }),
        redirect: "error", signal: AbortSignal.timeout(15_000),
      });
      if (!response.ok) throw new Error("Controller gateway unavailable");
      const decision = await response.json() as { action: string; instanceId: string; projectId: string; stopAt: number | null };
      if (decision.instanceId !== instanceId || decision.projectId !== projectId || !["start", "stop", "observe"].includes(decision.action))
        throw new Error("Invalid pinned controller decision");
      lastGateway = Date.now();
      if (decision.stopAt !== null) state.stopAt = decision.stopAt;
      else delete state.stopAt;
      await save();
      if (decision.action === "stop" && observed !== "STOPPED" && observed !== "STOPPING" &&
          (state.intent !== "stop" || Date.now() - (state.intentAt ?? 0) >= 30_000)) await mutate("stop");
      if (decision.action === "start" && observed === "STOPPED" && operationSettled && !state.intent) await mutate("start");
      // Persisted deadline is enforced locally too if the gateway disappears.
      if ((state.stopAt && Date.now() >= state.stopAt) || (state.stopAt && Date.now() - lastGateway > 45_000)) {
        if (observed !== "STOPPED" && observed !== "STOPPING" &&
            (state.intent !== "stop" || Date.now() - (state.intentAt ?? 0) >= 30_000)) await mutate("stop");
      }
    } catch {
      console.error("Compute observation or operation unavailable; no start is retried without reconciliation");
      // A controller with a recorded active session attempts provider stop even
      // when HTTP authorization/network is unavailable. Credentials never leave it.
      if (state.stopAt && (state.intent !== "stop" || Date.now() - (state.intentAt ?? 0) >= 30_000) && (Date.now() >= state.stopAt || Date.now() - lastGateway > 45_000)) {
        try { await mutate("stop"); } catch { console.error("Provider stop not confirmed; operator intervention required"); }
      }
    }
    await new Promise(resolve => setTimeout(resolve, 10_000));
  }
  // Graceful controller shutdown never leaves its known managed VM deliberately running.
  if (state.stopAt && state.intent !== "stop") { try { await mutate("stop"); } catch { console.error("Shutdown stop not confirmed"); } }
} finally { await sdk.close(); await unlink(lock); }
