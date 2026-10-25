import { expect, test } from 'bun:test';
import { assertMobileProfiles, mobileHarness } from './mobile-capabilities';
test('mobile offers every desktop harness, Nebius inference and sandbox team agents included', () => {
  expect(mobileHarness('codex')).toBe(true);
  expect(mobileHarness('nebius')).toBe(true);
  expect(mobileHarness(null)).toBe(false);
  const local = { provider: 'codex' as const, execution: { environment: 'local' as const, sandboxTarget: null, localFallback: false } };
  expect(() => assertMobileProfiles([local])).not.toThrow();
  expect(() => assertMobileProfiles([{ ...local, provider: 'nebius' }])).not.toThrow();
  expect(() => assertMobileProfiles([{ ...local, execution: { ...local.execution, environment: 'nebiusSandbox', sandboxTarget: 'target' } }])).not.toThrow();
  expect(() => assertMobileProfiles(undefined)).toThrow('verify the team');
});
