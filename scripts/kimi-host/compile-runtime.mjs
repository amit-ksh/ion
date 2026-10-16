// Compile the pinned native engine and copy its frozen production dependency graph.
import {
  readFile,
  writeFile,
  mkdir,
  readdir,
  stat,
  realpath,
} from "node:fs/promises";
import { existsSync, readFileSync, statSync, realpathSync } from "node:fs";
import { join, dirname, relative, resolve, sep, isAbsolute } from "node:path";
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { verifySource } from "./identity.mjs";
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
export const within = (parent, path) => {
  const rel = relative(parent, path);
  return (
    rel === "" ||
    (!rel.startsWith(".." + sep) && rel !== ".." && !isAbsolute(rel))
  );
};
const posix = (path) => path.split(sep).join("/");
const jsPath = (path) =>
  path
    .replace(/\.tsx?$/, ".js")
    .replace(/\.mts$/, ".mjs")
    .replace(/\.cts$/, ".cjs");
function metadata(data) {
  const rewrite = (value) =>
    typeof value === "string"
      ? jsPath(value)
      : Array.isArray(value)
        ? value.map(rewrite)
        : value && typeof value === "object"
          ? Object.fromEntries(
              Object.entries(value).map(([k, v]) => [k, rewrite(v)]),
            )
          : value;
  const copy = { ...data };
  for (const key of ["imports", "exports", "main", "module", "browser"])
    if (copy[key] !== undefined) copy[key] = rewrite(copy[key]);
  return copy;
}
export async function compileRuntime(
  source,
  engine,
  { reuseVerifiedDependencies = false } = {},
) {
  source = await realpath(source);
  verifySource(source);
  const inputs = JSON.parse(
    await readFile(new URL("./source-inputs.json", import.meta.url), "utf8"),
  );
  for (const [file, expected] of Object.entries(inputs.files)) {
    const path = resolve(source, file);
    if (!within(source, path) || hash(await readFile(path)) !== expected)
      throw Error("Pinned source input changed: " + file);
  }
  const transformer = createRequire(
    realpathSync(join(source, "node_modules/tsx/package.json")),
  );
  const esbuild = transformer("esbuild");
  if (esbuild.version !== "0.27.7")
    throw Error(
      "Use the frozen esbuild 0.27.7 transformer from the pinned Kimi dependency graph.",
    );
  const nodes = new Map(),
    slots = new Map(),
    sources = new Map();
  const compiled = [];
  const omitted = [];
  let bytes = 0,
    files = 0;
  await mkdir(join(engine, "node_modules"), { recursive: true });
  async function output(path, data) {
    bytes += data.length;
    files++;
    if (bytes > 2 * 1024 ** 3 || files > 100000)
      throw Error("Compiled host exceeds its package bounds.");
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, data);
  }
  const workspaceRoots = inputs.workspaces.map((rel) => ({
    rel,
    root: join(source, rel),
    data: JSON.parse(readFileSync(join(source, rel, "package.json"), "utf8")),
  }));
  for (const workspace of workspaceRoots) {
    const destination = join(engine, "node_modules", workspace.data.name);
    slots.set(destination, workspace.root);
    sources.set(workspace.root, workspace);
    workspace.out = destination;
  }
  function relativeImport(importer, spec) {
    if (!spec.startsWith(".")) {
      // Native source loading accepts extensionless published CJS subpaths.
      // Preserve conditional exports; add a suffix only if both resolutions
      // identify the exact same physical file in the frozen graph.
      if (!spec.startsWith("#") && !spec.startsWith("node:")) {
        const require = createRequire(importer);
        try {
          const exact = require.resolve(spec);
          for (const suffix of [".js", "/index.js"]) {
            try {
              if (require.resolve(spec + suffix) === exact)
                return spec + suffix;
            } catch {}
          }
        } catch {}
      }
      return spec;
    }
    const exact = resolve(dirname(importer), spec);
    const typed = /\.js$/.test(exact)
      ? [exact.slice(0, -3) + ".ts", exact.slice(0, -3) + ".tsx"]
      : [];
    for (const p of [
      exact,
      ...typed,
      ...[
        ".ts",
        ".tsx",
        ".mts",
        ".cts",
        ".js",
        ".mjs",
        ".cjs",
        "/index.ts",
        "/index.tsx",
        "/index.js",
        "/index.mjs",
      ].map((suffix) => exact + suffix),
    ]) {
      if (existsSync(p) && statSync(p).isFile()) {
        const rel = posix(relative(dirname(importer), jsPath(p)));
        return rel.startsWith(".") ? rel : "./" + rel;
      }
    }
    throw Error(
      "Unresolved native relative module: " +
        posix(relative(source, importer)) +
        " -> " +
        spec,
    );
  }
  for (const workspace of workspaceRoots) {
    const prefix = workspace.rel + "/";
    const entries = [];
    for (const file of Object.keys(inputs.files).filter((file) =>
      file.startsWith(prefix),
    )) {
      const rel = file.slice(prefix.length);
      if (
        !rel.startsWith("src/") &&
        !/^(?:assets|resources|data)\//.test(rel) &&
        !/^(?:package\.json|LICENSE[^/]*|NOTICE[^/]*|README[^/]*)$/i.test(rel)
      )
        continue;
      if (
        /(?:^|\/)(?:__tests__|test|tests|fixtures)\//.test(rel) ||
        /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(rel) ||
        /\.d\.[cm]?ts$/.test(rel)
      )
        continue;
      const from = join(source, file),
        to = join(workspace.out, jsPath(rel));
      if (/\.[cm]?[jt]sx?$/.test(rel)) {
        entries.push(from);
        continue;
      }
      const data = await readFile(from);
      await output(
        to,
        rel.endsWith("package.json")
          ? Buffer.from(
              JSON.stringify(metadata(JSON.parse(data.toString("utf8")))),
              "utf8",
            )
          : data,
      );
    }
    // Bundle only literal raw/JSON assets per source module. Other module boundaries
    // remain files, preserving import.meta asset locations and native DI identity.
    const result = await esbuild.build({
      entryPoints: entries,
      absWorkingDir: source,
      outdir: workspace.out,
      outbase: workspace.root,
      bundle: true,
      platform: "node",
      format: "esm",
      target: "node24",
      tsconfig: join(source, "tsconfig.json"),
      treeShaking: false,
      write: false,
      metafile: true,
      sourcemap: false,
      logLevel: "silent",
      plugins: [
        {
          name: "ion-native-module-boundaries",
          setup(build) {
            build.onResolve({ filter: /.*/ }, (args) => {
              if (args.kind === "entry-point") return;
              if (args.path.endsWith("?raw"))
                return {
                  path: resolve(args.resolveDir, args.path.slice(0, -4)),
                  namespace: "ion-raw",
                };
              if (args.path.startsWith(".") && args.path.endsWith(".json"))
                return {
                  path: resolve(args.resolveDir, args.path),
                  namespace: "ion-json",
                };
              return {
                path: relativeImport(args.importer, args.path),
                external: true,
              };
            });
            build.onLoad(
              { filter: /.*/, namespace: "ion-raw" },
              async (args) => ({
                contents: await readFile(args.path, "utf8"),
                loader: "text",
              }),
            );
            build.onLoad(
              { filter: /.*/, namespace: "ion-json" },
              async (args) => ({
                contents: await readFile(args.path, "utf8"),
                loader: "json",
              }),
            );
          },
        },
      ],
    });
    for (const file of result.outputFiles) {
      await output(file.path, file.contents);
      compiled.push(posix(relative(engine, file.path)));
    }
    await output(
      join(workspace.out, "package.json"),
      Buffer.from(JSON.stringify(metadata(workspace.data))),
    );
  }
  function sourceDependency(parent, name) {
    if (!/^(?:@[a-z0-9_.-]+\/)?[a-z0-9_.-]+$/i.test(name))
      throw Error("Invalid native dependency name.");
    const paths =
      createRequire(join(parent, "package.json")).resolve.paths(name) ?? [];
    for (const base of paths) {
      const candidate = join(base, name);
      if (
        !within(source, candidate) ||
        !existsSync(join(candidate, "package.json"))
      )
        continue;
      const exact = realpathSync(candidate);
      if (!within(source, exact))
        throw Error("Dependency escapes frozen checkout: " + name);
      return exact;
    }
    return undefined;
  }
  function destination(parent, name, wanted) {
    const paths =
      createRequire(join(parent, "package.json")).resolve.paths(name) ?? [];
    const candidates = paths
      .map((base) => join(base, name))
      .filter((path) => within(engine, path));
    const found = candidates.find((path) => slots.has(path));
    if (found && slots.get(found) === wanted) return found;
    return found ? candidates[0] : join(engine, "node_modules", name);
  }
  async function copyTree(root, folder, out, depth = 0) {
    if (depth > 80)
      throw Error("Native dependency tree depth exceeds its bound.");
    const entries = (await readdir(folder, { withFileTypes: true })).filter(
      (item) => !["node_modules", ".git"].includes(item.name),
    );
    for (let index = 0; index < entries.length; index += 8)
      await Promise.all(
        entries.slice(index, index + 8).map(async (item) => {
          const from = join(folder, item.name),
            to = join(out, item.name);
          if (item.isSymbolicLink()) {
            const exact = await realpath(from);
            if (!within(root, exact))
              throw Error("Dependency asset link escapes its package.");
            const info = await stat(from);
            if (info.isDirectory()) await copyTree(root, from, to, depth + 1);
            else if (info.isFile()) await output(to, await readFile(from));
          } else if (item.isDirectory())
            await copyTree(root, from, to, depth + 1);
          else if (item.isFile()) await output(to, await readFile(from));
        }),
      );
  }
  async function visit(root, out) {
    const key = root + "\0" + out;
    if (nodes.has(key)) return;
    const data = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
    nodes.set(key, {
      name: data.name,
      version: data.version,
      path: posix(relative(engine, out)),
      input: posix(relative(source, root)),
      license: data.license ?? null,
    });
    if (!sources.has(root) && !reuseVerifiedDependencies)
      await copyTree(root, root, out);
    const deps = {
      ...data.dependencies,
      ...data.optionalDependencies,
      ...data.peerDependencies,
    };
    for (const name of Object.keys(deps).sort()) {
      const target = sourceDependency(root, name);
      const optional =
        Object.hasOwn(data.optionalDependencies ?? {}, name) ||
        data.peerDependenciesMeta?.[name]?.optional === true;
      if (!target) {
        if (optional) {
          omitted.push({ package: data.name, dependency: name });
          continue;
        }
        throw Error(
          "Frozen runtime dependency missing: " + data.name + " -> " + name,
        );
      }
      const dest = destination(out, name, target);
      if (!slots.has(dest)) slots.set(dest, target);
      if (slots.get(dest) !== target)
        throw Error("Native dependency placement conflict.");
      await visit(target, dest);
    }
  }
  for (const workspace of workspaceRoots)
    await visit(workspace.root, workspace.out);
  // Preserve upstream notices even when a workspace relies on its repository's license.
  for (const name of ["LICENSE", "LICENSE.md", "NOTICE"])
    if (Object.hasOwn(inputs.files, name))
      await output(
        join(engine, "notices", name),
        await readFile(join(source, name)),
      );
  return {
    compiler: {
      esbuild: esbuild.version,
      target: "node24",
      modules: compiled.length,
    },
    packages: [...nodes.values()].sort((a, b) => a.path.localeCompare(b.path)),
    omittedOptional: omitted,
    sourceCommit: inputs.commit,
    sourceArchiveSha256: inputs.archiveSha256,
  };
}
