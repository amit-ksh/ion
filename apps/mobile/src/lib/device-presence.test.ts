import { expect, test } from 'bun:test';
import { mobileDeviceLabel } from './device-presence';

test('sends the Android model and an accurate fallback', () => {
  expect(mobileDeviceLabel('android', { Model: 'moto g32' })).toBe('moto g32');
  expect(mobileDeviceLabel('android', { Model: ' Pixel 9 ' })).toBe('Pixel 9');
  expect(mobileDeviceLabel('android', {})).toBe('Android');
  expect(mobileDeviceLabel('android', null)).toBe('Android');
  expect(mobileDeviceLabel('web', { Model: 'not a phone' })).toBe('Mobile device');
});

test('keeps native WebSocket header values bounded and printable', () => {
  expect(mobileDeviceLabel('android', { Model: '\r\n\t' })).toBe('Android');
  expect(mobileDeviceLabel('android', { Model: 'phone\r\nInjected: value' })).toBe('phoneInjected: value');
  expect(mobileDeviceLabel('android', { Model: 'x'.repeat(1000) })).toHaveLength(64);
  expect(mobileDeviceLabel('android', { Model: 7 })).toBe('Android');
});
