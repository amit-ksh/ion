import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

test('OS app identity and permission descriptions use ion', () => {
  const app = readFileSync('resources/Info.plist', 'utf8').replaceAll('\r\n', '\n');
  const helper = readFileSync('resources/computer-use/Info.plist', 'utf8').replaceAll('\r\n', '\n');
  expect(app).toContain('<key>CFBundleDisplayName</key>\n    <string>ion</string>');
  expect(app).toContain('ion records your voice');
  for (const field of ['CFBundleDisplayName', 'CFBundleExecutable', 'CFBundleName']) {
    expect(helper).toContain(`<key>${field}</key>\n    <string>ion Computer Use</string>`);
  }
  expect(helper).toContain('ion uses Accessibility access');
  expect(helper).toContain('ion uses Screen Recording access');
  expect(helper).not.toContain('Waku');
  expect(helper).toContain('sh.waku.computer-use');
  expect(readFileSync('build.rs', 'utf8')).toContain('let description = "ion";');
  const mobile = JSON.parse(readFileSync('apps/mobile/app.json', 'utf8'));
  expect(mobile.expo.name).toBe('Ion');
  expect(JSON.stringify(mobile.expo.plugins)).toContain('ion');
  expect(readFileSync('crates/waku-computer-use/src/sdk.rs', 'utf8')).toContain(
    'Cua Driver SDK is missing from this ion build',
  );
  expect(readFileSync('crates/waku-core/src/claude_session.rs', 'utf8')).toContain(
    '"ion rewind"',
  );
});
