import { expect, test } from 'bun:test';
import { localTimeLabel, relativeTimeLabel } from './time-labels';

const now = new Date(2026, 9, 24, 12, 0, 0).getTime();
const at = (date: Date) => Math.floor(date.getTime() / 1_000);
const seconds = Math.floor(now / 1_000);

test('relative labels read past and future without raw timestamps', () => {
  expect(relativeTimeLabel(seconds - 10, now)).toBe('just now');
  expect(relativeTimeLabel(seconds - 300, now)).toBe('5m ago');
  expect(relativeTimeLabel(seconds - 3 * 3_600, now)).toBe('3h ago');
  expect(relativeTimeLabel(seconds - 2 * 86_400, now)).toBe('2d ago');
  expect(relativeTimeLabel(seconds + 90, now)).toBe('in 1m');
  expect(relativeTimeLabel(seconds + 2 * 3_600, now)).toBe('in 2h');
  expect(relativeTimeLabel(0, now)).toBe('Unknown time');
});

test('local labels use the phone clock and nearby day names', () => {
  expect(localTimeLabel(at(new Date(2026, 9, 24, 17, 5)), now)).toBe('Today 17:05');
  expect(localTimeLabel(at(new Date(2026, 9, 25, 9, 0)), now)).toBe('Tomorrow 09:00');
  expect(localTimeLabel(at(new Date(2026, 9, 23, 8, 30)), now)).toBe('Yesterday 08:30');
  expect(localTimeLabel(at(new Date(2026, 10, 2, 9, 0)), now)).toBe('Nov 2, 09:00');
  expect(localTimeLabel(at(new Date(2027, 0, 3, 9, 0)), now)).toBe('Jan 3, 2027');
  expect(localTimeLabel(Number.NaN, now)).toBe('Unknown time');
});
