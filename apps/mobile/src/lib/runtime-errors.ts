const DAEMON_DISCONNECT_MESSAGES = new Set([
  // Existing saved errors retain the pre-ion text after an upgrade.
  'Waku daemon disconnected',
  'Waku daemon is disconnected',
  'Waku client disconnected',
  'ion daemon disconnected',
  'ion daemon is disconnected',
  'ion client disconnected',
]);

/** A failed optional MCP connection must not be presented as an agent startup failure. */
export function summarizeRuntimeError(message: string): string {
  const server = message.match(/(?:^|\n)(?:[^:\n]+:\s*)?MCP client for `([^`\n]{1,64})` failed to start:/)?.[1];
  if (server) return `${server} couldn’t connect. Check its setup on your desktop.`;
  return /<(?:!doctype|html)(?:\s|>)/i.test(message)
    ? 'Your agent couldn’t connect. Check its setup on your desktop, then try again.'
    : message;
}

/** Connection loss is owned by the global reconnect banner. It must not
 * survive as a task-level error once that connection has recovered. */
export function isDaemonDisconnectError(cause: unknown): boolean {
  const message = cause instanceof Error ? cause.message : cause;
  return typeof message === 'string' && DAEMON_DISCONNECT_MESSAGES.has(message.trim());
}

/** Drops only stale transport errors and preserves the input reference when
 * there is nothing to remove, so reconnects without errors do not re-render. */
export function clearDaemonDisconnectErrors(
  errors: Record<string, string | undefined>,
): Record<string, string | undefined> {
  let next: Record<string, string | undefined> | null = null;
  for (const [sessionId, error] of Object.entries(errors)) {
    if (!isDaemonDisconnectError(error)) continue;
    next ??= { ...errors };
    delete next[sessionId];
  }
  return next ?? errors;
}
