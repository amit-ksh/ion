export type DockerCommand = (args: string[], seconds?: number, maximum?: number) => Promise<string>;
/** The database's running bit does not survive a real VM power cycle. Inspect
 * the owned container and reconcile actual Docker state before claiming ready. */
export async function reconcileRuntime(id: string, want: boolean, command: DockerCommand): Promise<"running" | "stopped"> {
  if (!/^[A-Za-z0-9_-]{32}$/.test(id)) throw new Error("Invalid owned runtime");
  const name = "ion-" + id;
  const running = (await command(["inspect", "--format", "{{.State.Running}}", name], 10)).trim();
  if (!["true", "false"].includes(running)) throw new Error("Runtime state unavailable");
  if (want && running === "false") await command(["start", name], 30);
  if (!want && running === "true") await command(["stop", "--time=10", name], 20);
  return want ? "running" : "stopped";
}
