import { expect, test } from "bun:test";
import { fileURLToPath } from "node:url";

test("phone dictation hook handles lifecycle and native stop failure without transferring discarded audio", () => {
  const child = Bun.spawnSync(
    [
      process.execPath,
      "run",
      "--tsconfig-override",
      "apps/mobile/tsconfig.json",
      "apps/mobile/src/hooks/__fixtures__/phone-dictation-lifecycle.ts",
    ],
    {
      cwd: fileURLToPath(new URL("../../../../", import.meta.url)),
      stdout: "pipe",
      stderr: "pipe",
      timeout: 20_000,
    },
  );
  expect({
    exitCode: child.exitCode,
    output: child.stdout.toString(),
    error: child.stderr.toString(),
  }).toMatchObject({ exitCode: 0, error: "" });
  expect(JSON.parse(child.stdout.toString())).toMatchObject({
    checks: 10,
    transfers: 2,
    microphoneUsed: false,
  });
});
