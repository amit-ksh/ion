/** Optional MCP failures must not be presented as agent startup failures. */
export function summarizeRuntimeError(message: string): string {
  const server = message.match(/(?:^|\n)(?:[^:\n]+:\s*)?MCP client for `([^`\n]{1,64})` failed to start:/)?.[1];
  if (server) return `${server} couldn’t connect. Check its setup on your desktop.`;
  return /<(?:!doctype|html)(?:\s|>)/i.test(message)
    ? 'Your agent couldn’t connect. Check its setup on your desktop, then try again.'
    : message;
}
