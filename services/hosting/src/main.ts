import { reconcileRuntime } from "./reconcile-runtime";
import { startRegistryProxy } from "./registry-proxy";
/** Dedicated Linux build/runtime worker. Never run on the control or speech host. */
import { Database } from "bun:sqlite";
import {
  mkdir,
  writeFile,
  readFile,
  readdir,
  lstat,
  realpath,
  chmod,
  rm,
} from "node:fs/promises";
import { join, resolve, relative, isAbsolute } from "node:path";
import { createHash } from "node:crypto";
import type {
  BundleFile,
  BuildSettings,
} from "../../../packages/ion-cloud/src/hosting";
const digest = (value: Uint8Array | string) =>
  createHash("sha256").update(value).digest("hex");
if (process.platform !== "linux")
  throw new Error("Hosting worker requires a dedicated Linux host");
process.umask(0o077);
const origin = process.env.ION_CLOUD_PUBLIC_ORIGIN ?? "",
  token = process.env.ION_HOSTING_WORKER_TOKEN ?? "",
  root = process.env.ION_HOSTING_STATE_DIRECTORY ?? "";
const network = process.env.ION_HOSTING_NETWORK ?? "ion-apps";
if (
  new URL(origin).protocol !== "https:" ||
  !/^[A-Za-z0-9_-]{43,128}$/.test(token) ||
  !isAbsolute(root) ||
  process.env.ION_HOSTING_ISOLATION_READY !== "gvisor-private-network-v1"
)
  throw new Error(
    "Configure private worker, verified isolation and API origin",
  );
const images: Record<string, string> = {
  static: process.env.ION_HOSTING_NODE_IMAGE ?? "",
  node: process.env.ION_HOSTING_NODE_IMAGE ?? "",
  bun: process.env.ION_HOSTING_BUN_IMAGE ?? "",
  python: process.env.ION_HOSTING_PYTHON_IMAGE ?? "",
};
for (const image of Object.values(images))
  if (!/^[a-z0-9][a-z0-9./:_-]+@sha256:[a-f0-9]{64}$/.test(image))
    throw new Error("Pin each accepted runtime image by digest");
await mkdir(root, { recursive: true, mode: 0o700 });
const rootInfo = await lstat(root);
if (rootInfo.isSymbolicLink() || rootInfo.mode & 0o077)
  throw new Error("Worker state must be private");
const db = new Database(join(root, "worker.sqlite"), {
  create: true,
  strict: true,
});
db.run("PRAGMA synchronous=FULL");
db.run(
  "CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, workspace TEXT NOT NULL, state TEXT NOT NULL, port INTEGER, receipt TEXT)",
);
type Job = {
  workspace: string;
  release: { id: string; settings: BuildSettings };
  files: BundleFile[];
  environment?: Record<string, string>;
};
const registryHost =
  process.env.ION_HOSTING_REGISTRY_PROXY_HOST ?? "172.31.0.1";
const registryPort = Number(
  process.env.ION_HOSTING_REGISTRY_PROXY_PORT ?? 3128,
);
const registry = startRegistryProxy(registryHost, registryPort);
let stopping = false;
async function command(
  args: string[],
  seconds = 60,
  maximum = 500_000,
): Promise<string> {
  const proc = Bun.spawn(["docker", ...args], {
    stdout: "pipe",
    stderr: "pipe",
    env: {
      PATH: process.env.PATH ?? "/usr/bin:/bin",
      HOME: process.env.HOME ?? root,
    },
  });
  let output = "";
  let size = 0;
  let expired = false;
  const timer = setTimeout(() => {
    expired = true;
    proc.kill();
  }, seconds * 1000);
  const collect = async (stream: ReadableStream<Uint8Array>) => {
    for await (const chunk of stream) {
      size += chunk.length;
      if (size > maximum) {
        proc.kill();
        throw new Error("Worker output limit");
      }
      output += new TextDecoder().decode(chunk);
    }
  };
  try {
    await Promise.all([collect(proc.stdout), collect(proc.stderr)]);
    if ((await proc.exited) !== 0 || expired)
      throw new Error("Isolated command failed");
    return output;
  } finally {
    clearTimeout(timer);
  }
}
async function api(
  path: string,
  body: unknown,
  timeout = 30_000,
): Promise<any> {
  const response = await fetch(origin + path, {
    method: "POST",
    headers: {
      Authorization: "Bearer " + token,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
    redirect: "error",
    signal: AbortSignal.timeout(timeout),
  });
  if (!response.ok) throw new Error("Control API unavailable");
  return response.json();
}
const idValid = (id: string) => /^[A-Za-z0-9_-]{32}$/.test(id);
const container = (id: string) => "ion-" + id;
async function remove(id: string) {
  const name = container(id),
    exists = await command(
      ["ps", "-a", "--filter", "name=^/" + name + "$", "--format", "{{.ID}}"],
      10,
    );
  if (exists.trim()) await command(["rm", "-f", name], 15);
  const remaining = await command(
    ["ps", "-a", "--filter", "name=^/" + name + "$", "--format", "{{.ID}}"],
    10,
  );
  if (remaining.trim())
    throw new Error("Runtime removal could not be confirmed");
}
const sourcePath = (path: string) =>
  path.length <= 240 &&
  !path.startsWith("/") &&
  !path.includes("\\") &&
  path
    .split("/")
    .every((v) => v && v !== "." && v !== ".." && !v.startsWith("."));
async function exportFiles(directory: string): Promise<BundleFile[]> {
  const base = await realpath(directory),
    output: BundleFile[] = [];
  let total = 0;
  async function visit(directory: string) {
    for (const item of await readdir(directory, { withFileTypes: true })) {
      const path = join(directory, item.name),
        info = await lstat(path);
      if (info.isSymbolicLink())
        throw new Error("Build output contains a symlink");
      if (info.isDirectory()) {
        await visit(path);
        continue;
      }
      if (!info.isFile())
        throw new Error("Build output contains a special file");
      const name = relative(base, path).replaceAll("\\", "/");
      if (!sourcePath(name) || info.size > 8_000_000)
        throw new Error("Invalid build output");
      const content = await readFile(path);
      total += content.length;
      if (total > 20_000_000 || output.length >= 500)
        throw new Error("Build output limit");
      output.push({
        path: name,
        bytes: content.length,
        sha256: digest(content),
        content: content.toString("base64"),
      });
    }
  }
  await visit(base);
  return output;
}
async function execute(job: Job) {
  const id = job.release.id,
    settings = job.release.settings;
  if (
    !idValid(id) ||
    !/^[a-f0-9]{64}$/.test(job.workspace) ||
    !images[settings.runtime] ||
    !Array.isArray(job.files)
  )
    throw new Error("Invalid admitted job");
  if (
    db.query<{ n: number }, []>("SELECT count(*) AS n FROM jobs").get()!.n >= 20
  )
    throw new Error("Worker retained release capacity reached");
  try {
    db.run("INSERT INTO jobs(id,workspace,state) VALUES(?,?,'building')", [
      id,
      job.workspace,
    ]);
  } catch {
    return;
  }
  const directory = join(root, id),
    source = join(directory, "source"),
    output = join(directory, "output");
  await mkdir(source, { recursive: true, mode: 0o755 });
  await mkdir(output, { recursive: true, mode: 0o777 });
  await chmod(directory, 0o755);
  await chmod(source, 0o755);
  await chmod(output, 0o777);
  let bytes = 0;
  try {
    for (const file of job.files) {
      if (!sourcePath(file.path)) throw new Error("Invalid source path");
      const decoded = Buffer.from(file.content, "base64");
      bytes += decoded.length;
      if (
        bytes > 20_000_000 ||
        decoded.length !== file.bytes ||
        digest(decoded) !== file.sha256
      )
        throw new Error("Changed source");
      const target = join(source, file.path);
      let parent = source;
      for (const component of file.path.split("/").slice(0, -1)) {
        parent = join(parent, component);
        await mkdir(parent, { recursive: true, mode: 0o755 });
        await chmod(parent, 0o755);
      }
      await writeFile(target, decoded, { mode: 0o644, flag: "wx" });
      await chmod(target, 0o644);
    }
    const common = [
      "--runtime=runsc",
      "--user=1000:1000",
      "--cap-drop=ALL",
      "--security-opt=no-new-privileges",
      "--read-only",
      "--pids-limit=128",
      "--memory=1g",
      "--memory-swap=1g",
      "--cpus=1",
      "--ulimit=nofile=1024:1024",
      "--ulimit=nproc=128:128",
      "--log-driver=local",
      "--log-opt=max-size=1m",
      "--log-opt=max-file=2",
    ];
    const needsBuild =
      settings.build.length > 0 ||
      ["node", "bun", "python"].includes(settings.runtime);
    if (needsBuild) {
      const buildName = "ion-build-" + id;
      const proxy = "http://" + registryHost + ":" + registryPort;
      const buildScript =
        'mkdir -p /tmp/ion-work && cd /tmp/ion-work && cp -R /source/. . && if [ -f package-lock.json ]; then npm ci --no-audit --no-fund; elif [ -f bun.lock ]; then bun install --frozen-lockfile; elif [ -f requirements.txt ]; then python -m pip install --target=/tmp/ion-work/.ion-python -r requirements.txt; fi && "$@" && mkdir -p /tmp/ion-out && if [ "$ION_RUNTIME" = static ]; then cp -R "$ION_OUTPUT"/. /tmp/ion-out/; else cp -R /tmp/ion-work/. /tmp/ion-out/; fi';
      try {
        // A quota-limited writable layer retains output after exit. overlay2 on
        // XFS with pquota is required; unsupported workers fail the job closed.
        await command(
          [
            "run",
            "--name",
            buildName,
            ...common.filter((v) => v !== "--read-only"),
            "--storage-opt=size=1G",
            "--network=" + network,
            "--mount",
            "type=bind,src=" + source + ",dst=/source,readonly",
            "--env",
            "ION_OUTPUT=" + settings.outputDirectory,
            "--env",
            "ION_RUNTIME=" + settings.runtime,
            "--env",
            "HTTPS_PROXY=" + proxy,
            "--env",
            "HTTP_PROXY=" + proxy,
            "--env",
            "https_proxy=" + proxy,
            "--env",
            "http_proxy=" + proxy,
            "--env",
            "npm_config_cache=/tmp/npm-cache",
            "--env",
            "BUN_INSTALL_CACHE_DIR=/tmp/bun-cache",
            "--workdir=/tmp",
            images[settings.runtime]!,
            "/bin/sh",
            "-c",
            buildScript,
            "ion-build",
            ...(settings.build.length ? settings.build : ["true"]),
          ],
          600,
        );
        await command(["cp", buildName + ":/tmp/ion-out/.", output], 30);
      } finally {
        try {
          await command(["rm", "-f", buildName], 15);
        } catch {}
      }
    }
    if (settings.runtime === "static") {
      const files = settings.build.length
        ? await exportFiles(output)
        : job.files;
      const receipt = {
        workspace: job.workspace,
        releaseId: id,
        success: true,
        files,
      };
      db.run("UPDATE jobs SET state='built',receipt=? WHERE id=?", [
        JSON.stringify(receipt),
        id,
      ]);
      try {
        await api("/internal/hosting/complete", receipt);
      } catch {
        /* Reconcile this committed receipt; never replay a build. */
      }
      return;
    }
    if (
      db
        .query<{ n: number }, []>(
          "SELECT count(*) AS n FROM jobs WHERE state='running'",
        )
        .get()!.n >= 4
    )
      throw new Error("Runtime capacity reached");
    const runtimeSource = needsBuild ? output : source;
    const envPath = join(directory, "runtime.env"),
      entries = Object.entries(job.environment ?? {});
    if (
      entries.some(
        ([key, value]) =>
          !/^[_A-Za-z][_A-Za-z0-9]{0,63}$/.test(key) ||
          typeof value !== "string" ||
          /[\r\n\0]/.test(value) ||
          value.length > 8000,
      ) ||
      entries.length > 32
    )
      throw new Error("Invalid runtime secrets");
    await writeFile(
      envPath,
      ["PORT=" + settings.port, ...entries.map(([k, v]) => k + "=" + v)].join(
        "\n",
      ),
      { mode: 0o600 },
    );
    await command(
      [
        "create",
        "--name",
        container(id),
        ...common,
        "--network=" + network,
        "--tmpfs=/tmp:rw,noexec,nosuid,size=64m",
        "--mount",
        "type=bind,src=" + runtimeSource + ",dst=/app,readonly",
        "--env-file",
        envPath,
        "--env",
        "PYTHONPATH=/app/.ion-python",
        "--env",
        "PYTHONDONTWRITEBYTECODE=1",
        "--workdir=/app",
        "--publish",
        "127.0.0.1::" + settings.port,
        images[settings.runtime]!,
        ...settings.start,
      ],
      30,
    );
    await rm(envPath, { force: true });
    await command(["start", container(id)], 30);
    const portValue = (
      await command(["port", container(id), String(settings.port)], 10)
    ).trim();
    const match = /^127\.0\.0\.1:(\d+)$/.exec(portValue);
    if (!match) throw new Error("Runtime port unavailable");
    const port = Number(match[1]);
    let healthy = false;
    for (let attempt = 0; attempt < 30 && !stopping; attempt++) {
      try {
        const response = await fetch(
          "http://127.0.0.1:" + port + settings.healthPath,
          { redirect: "error", signal: AbortSignal.timeout(1000) },
        );
        if (response.ok) {
          healthy = true;
          break;
        }
      } catch {}
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    if (!healthy) throw new Error("Runtime readiness failed");
    const receipt = { workspace: job.workspace, releaseId: id, success: true };
    db.run("UPDATE jobs SET state='running',port=?,receipt=? WHERE id=?", [
      port,
      JSON.stringify(receipt),
      id,
    ]);
    try {
      await api("/internal/hosting/complete", receipt);
    } catch {
      /* Keep the healthy runtime and retry only its receipt. */
    }
  } catch {
    await remove(id);
    const receipt = { workspace: job.workspace, releaseId: id, success: false };
    db.run("UPDATE jobs SET state='failed',receipt=? WHERE id=?", [
      JSON.stringify(receipt),
      id,
    ]);
    try {
      await api("/internal/hosting/complete", receipt);
    } catch {}
  }
}
const dockerInfo = JSON.parse(
  await command(["info", "--format", "{{json .}}"], 15),
) as {
  Driver: string;
  DockerRootDir: string;
  DriverStatus?: string[][];
  Runtimes: Record<string, unknown>;
};
if (
  dockerInfo.Driver !== "overlay2" ||
  !dockerInfo.DriverStatus?.some(
    (v) => v[0] === "Backing Filesystem" && v[1] === "xfs",
  ) ||
  !dockerInfo.Runtimes.runsc
)
  throw new Error("Worker requires gVisor and quota-enabled overlay2/XFS");
const mounts = (await readFile("/proc/mounts", "utf8"))
  .split("\n")
  .map((line) => line.split(" "))
  .filter((parts) => parts[1] && dockerInfo.DockerRootDir.startsWith(parts[1]))
  .sort((a, b) => b[1]!.length - a[1]!.length);
if (
  !mounts[0]?.[3]
    ?.split(",")
    .some((value) => value === "prjquota" || value === "pquota")
)
  throw new Error(
    "Enable XFS project quotas for Docker before starting the worker",
  );
await command(["info", "--format", "{{json .Runtimes}}"], 15).then((value) => {
  if (!value.includes('"runsc"')) throw new Error("gVisor runtime is required");
});
await command(
  ["network", "inspect", network, "--format", "{{.Internal}}"],
  15,
).then((value) => {
  if (value.trim() !== "true")
    throw new Error("Runtime network must be internal");
});
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(process.env.ION_HOSTING_WORKER_PORT ?? 8792),
  maxRequestBodySize: 2_000_000,
  async fetch(request) {
    if (
      digest(request.headers.get("authorization") ?? "") !==
      digest("Bearer " + token)
    )
      return new Response("Unauthorized", { status: 401 });
    const url = new URL(request.url);
    const log = /^\/logs\/([A-Za-z0-9_-]{32})$/.exec(url.pathname);
    if (log && request.method === "GET") {
      const record = db
        .query<{ state: string }, [string]>("SELECT state FROM jobs WHERE id=?")
        .get(log[1]!);
      if (!record) return new Response("Not found", { status: 404 });
      try {
        const info = JSON.parse(
          await command(
            ["inspect", container(log[1]!), "--format", "{{json .Config.Env}}"],
            10,
          ),
        ) as string[];
        let logs = await command(
          ["logs", "--tail=100", "--since=1h", container(log[1]!)],
          10,
          100_000,
        );
        for (const entry of info) {
          const index = entry.indexOf("="),
            value = entry.slice(index + 1);
          if (value.length >= 4) logs = logs.split(value).join("[redacted]");
        }
        logs = logs
          .replace(/\u001b\[[0-9;]*[A-Za-z]/g, "")
          .replace(
            /(bearer\s+|(?:api[_-]?key|token|password|secret)\s*[:=]\s*)[^\s,;]+/gi,
            "$1[redacted]",
          )
          .slice(-20_000);
        return Response.json({ logs });
      } catch {
        return new Response("Runtime logs unavailable", { status: 503 });
      }
    }
    const match = /^\/runtime\/([A-Za-z0-9_-]{32})(\/.*)?$/.exec(url.pathname);
    if (!match) return new Response("Not found", { status: 404 });
    const record = db
      .query<{ port: number; state: string }, [string]>(
        "SELECT port,state FROM jobs WHERE id=?",
      )
      .get(match[1]!);
    if (!record || record.state !== "running")
      return new Response("Runtime stopped", { status: 503 });
    const headers = new Headers(request.headers);
    for (const name of [
      "authorization",
      "cookie",
      "host",
      "connection",
      "upgrade",
      "forwarded",
      "x-forwarded-for",
      "x-forwarded-host",
      "x-forwarded-proto",
    ])
      headers.delete(name);
    try {
      const response = await fetch(
        "http://127.0.0.1:" + record.port + (match[2] ?? "/") + url.search,
        {
          method: request.method,
          headers,
          body: request.body,
          redirect: "manual",
          signal: AbortSignal.timeout(30_000),
          duplex: "half",
        } as RequestInit,
      );
      const output = new Headers(response.headers);
      output.delete("set-cookie");
      output.delete("server");
      output.set("Cache-Control", "no-store");
      return new Response(response.body, {
        status: response.status,
        headers: output,
      });
    } catch {
      return new Response("Runtime unavailable", { status: 503 });
    }
  },
});
let building: string | null = null;
let activeBuild: Promise<void> | null = null;
const loop = (async () => {
  while (!stopping) {
    try {
      const desired = await api("/internal/hosting/desired", {});
      const requested = new Map<string, boolean>(
        desired.jobs.map((job: any) => [job.id, job.running]),
      );
      if (
        building &&
        !desired.jobs.some(
          (job: any) => job.id === building && (job.building || job.running),
        )
      ) {
        try {
          await command(["rm", "-f", "ion-build-" + building], 15);
        } catch {}
        const current = db
          .query<{ state: string }, [string]>(
            "SELECT state FROM jobs WHERE id=?",
          )
          .get(building);
        if (current?.state === "running") {
          await command(["stop", "--time=10", container(building)], 20);
          db.run("UPDATE jobs SET state='stopped' WHERE id=?", [building]);
        } else {
          await remove(building);
        }
      }
      const jobs = db
        .query<{ id: string; state: string; receipt: string | null }, []>(
          "SELECT id,state,receipt FROM jobs",
        )
        .all();
      for (const job of jobs) {
        if (!requested.has(job.id)) {
          try {
            await command(["rm", "-f", "ion-build-" + job.id], 15);
          } catch {}
          await remove(job.id);
          if (idValid(job.id)) {
            const target = resolve(root, job.id);
            if (target.startsWith(resolve(root) + "/"))
              await rm(target, { recursive: true, force: true });
          }
          db.run("DELETE FROM jobs WHERE id=?", [job.id]);
          continue;
        }
        if (job.receipt) {
          try {
            await api("/internal/hosting/complete", JSON.parse(job.receipt));
          } catch {}
        }
        if (["running", "stopped"].includes(job.state)) {
          const want = requested.get(job.id) === true;
          try {
            const actual = await reconcileRuntime(job.id, want, command);
            db.run("UPDATE jobs SET state=? WHERE id=?", [actual, job.id]);
          } catch {
            const workspace = db.query<{ workspace: string }, [string]>("SELECT workspace FROM jobs WHERE id=?").get(job.id)!.workspace;
            const receipt = { workspace, releaseId: job.id, uncertain: true };
            db.run("UPDATE jobs SET state='uncertain',receipt=? WHERE id=?", [JSON.stringify(receipt), job.id]);
            try { await api("/internal/hosting/complete", receipt); } catch {}
          }
        } else if (job.state === "building" && job.id !== building) {
          await remove(job.id);
          const receipt = {
            workspace: db
              .query<{ workspace: string }, [string]>(
                "SELECT workspace FROM jobs WHERE id=?",
              )
              .get(job.id)!.workspace,
            releaseId: job.id,
            uncertain: true,
          };
          db.run("UPDATE jobs SET state='uncertain',receipt=? WHERE id=?", [
            JSON.stringify(receipt),
            job.id,
          ]);
        }
      }
      if (!activeBuild) {
        const response = await api("/internal/hosting/claim", {});
        if (response.job) {
          building = response.job.release.id;
          activeBuild = execute(response.job)
            .catch(() => {})
            .finally(() => {
              building = null;
              activeBuild = null;
            });
        }
      }
    } catch {
      console.error("Hosting worker connection or operation unavailable");
    }
    await new Promise((resolve) => setTimeout(resolve, 3000));
  }
})();
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.on(signal, () => {
    stopping = true;
  });
await loop;
if (activeBuild) await activeBuild;
await server.stop();
registry.close();
db.close();
