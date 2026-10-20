import { describe, expect, test } from 'bun:test';

import {
  clearDaemonDisconnectErrors,
  isDaemonDisconnectError,
} from './runtime-errors';

describe('isDaemonDisconnectError', () => {
  test('recognizes the client transport messages emitted during a dropped link', () => {
    expect(isDaemonDisconnectError(new Error('ion daemon disconnected'))).toBe(true);
    expect(isDaemonDisconnectError('ion daemon is disconnected')).toBe(true);
    expect(isDaemonDisconnectError('ion client disconnected')).toBe(true);
    expect(isDaemonDisconnectError('Waku daemon disconnected')).toBe(true);
    expect(isDaemonDisconnectError('Waku daemon is disconnected')).toBe(true);
    expect(isDaemonDisconnectError('Waku client disconnected')).toBe(true);
  });

  test('does not swallow a real daemon or agent error', () => {
    expect(isDaemonDisconnectError('timed out waiting for ion daemon')).toBe(false);
    expect(isDaemonDisconnectError('Provider process exited')).toBe(false);
    expect(isDaemonDisconnectError(null)).toBe(false);
  });
});

describe('clearDaemonDisconnectErrors', () => {
  test('removes only stale transport errors after reconnecting', () => {
    const errors = {
      disconnected: 'ion daemon disconnected',
      provider: 'Provider process exited',
    };

    expect(clearDaemonDisconnectErrors(errors)).toEqual({
      provider: 'Provider process exited',
    });
  });

  test('preserves the record when there is nothing to clear', () => {
    const errors = { provider: 'Provider process exited' };
    expect(clearDaemonDisconnectErrors(errors)).toBe(errors);
  });
});
