import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, isAbsolute } from "node:path";
export const version = "0.1.0";
export const identity =
  "ion-kimi-host/0.1.0 kimi/2.2.0 source/472192c34c1efad89a369870eff0da0b708ce9bb protocol/1";
export function verifySource(source) {
  if (!isAbsolute(source))
    throw Error("Use an absolute pinned Kimi source directory.");
  for (const [file, hash] of [
    [
      "tsconfig.json",
      "528a14471026a89f3e9038c0dc0ef99958dbbcedba9d97a64b0a6ff6216cd87e",
    ],
    [
      "pnpm-lock.yaml",
      "a30a1ad0bd2ca52e6f9e8d2116c0c4aa7a655e1a3c3a3fc309360294ea9e9639",
    ],
    [
      "packages/acp-server/src/start.ts",
      "b4a460764ce8e999cd7f6798bf07a1293cf353ac741aec5e7c243bd86f71805f",
    ],
    [
      "packages/agent-core-v2/src/agent/toolExecutor/toolExecutorService.ts",
      "c9b0a6f0ba91a9908dfceef482f1e76e98d8e69800a1921ce5869dbdd9e59b3b",
    ],
  ])
    if (
      createHash("sha256")
        .update(readFileSync(join(source, file)))
        .digest("hex") !== hash
    )
      throw Error(
        "Kimi source contract identity changed; prepare the pinned source again.",
      );
  if (
    JSON.parse(
      readFileSync(join(source, "apps/kimi-code/package.json"), "utf8"),
    ).version !== "2.2.0" ||
    JSON.parse(
      readFileSync(join(source, "node_modules/tsx/package.json"), "utf8"),
    ).version !== "4.21.0"
  )
    throw Error("Kimi source or locked loader version changed.");
}
