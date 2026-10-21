import { expect, test } from 'bun:test';
import { summarizeRuntimeError } from './runtime-error-summary';

test('names the failed optional MCP integration instead of blaming a working agent', () => {
  expect(summarizeRuntimeError('GitLab: MCP client for `GitLab` failed to start: HTTP 422: <!DOCTYPE html><html>Rejected</html>'))
    .toBe('GitLab couldn’t connect. Check its setup on your desktop.');
  expect(summarizeRuntimeError('MCP client for `Calendar` failed to start: authentication required'))
    .toBe('Calendar couldn’t connect. Check its setup on your desktop.');
});

test('replaces HTML agent failures and preserves useful plain diagnostics', () => {
  expect(summarizeRuntimeError('HTTP 502: <html>Unavailable</html>'))
    .toBe('Your agent couldn’t connect. Check its setup on your desktop, then try again.');
  expect(summarizeRuntimeError('Codex did not finish opening its thread'))
    .toBe('Codex did not finish opening its thread');
  expect(summarizeRuntimeError('MCP startup incomplete')).toBe('MCP startup incomplete');
});
