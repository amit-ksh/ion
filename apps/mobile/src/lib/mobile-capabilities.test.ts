import { expect, test } from 'bun:test';
import { assertMobileProfiles, mobileHarness } from './mobile-capabilities';
test('mobile excludes cloud harnesses and rejects pinned remote team execution', () => {
  expect(mobileHarness('codex')).toBe(true);
  expect(mobileHarness('nebius')).toBe(false);
  expect(mobileHarness(null)).toBe(false);
  const local = { provider: 'codex' as const, execution: { environment: 'local' as const, sandboxTarget: null, localFallback: false } };
  expect(() => assertMobileProfiles([local])).not.toThrow();
  expect(() => assertMobileProfiles([{ ...local, provider: 'nebius' }])).toThrow('cloud inference');
  expect(() => assertMobileProfiles([{ ...local, execution: { ...local.execution, environment: 'nebiusSandbox', sandboxTarget: 'target' } }])).toThrow('cloud inference');
  expect(() => assertMobileProfiles(undefined)).toThrow('verify the team');
});
