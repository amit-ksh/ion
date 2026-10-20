import { expect, test } from 'bun:test';
import { readFileSync, existsSync } from 'node:fs';

const source = readFileSync('apps/mobile/src/constants/theme.ts', 'utf8');
function palette(mode: 'light' | 'dark') {
  const body = source.split(`  ${mode}: {`)[1]!.split('\n  },')[0]!;
  return Object.fromEntries([...body.matchAll(/(\w+): '(#[a-fA-F0-9]{6})'/g)].map(match => [match[1]!, match[2]!]));
}
function luminance(color: string) {
  const channels = [1, 3, 5].map(offset => parseInt(color.slice(offset, offset + 2), 16) / 255)
    .map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
  return channels[0]! * .2126 + channels[1]! * .7152 + channels[2]! * .0722;
}
function contrast(a: string, b: string) { const values = [luminance(a), luminance(b)].sort((x, y) => y - x); return (values[0]! + .05) / (values[1]! + .05); }
for (const mode of ['light', 'dark'] as const) {
  test(`${mode} text and primary action roles retain AA contrast across the workbench`, () => {
    const p = palette(mode);
    for (const foreground of ['text', 'textSecondary', 'textTertiary', 'textGhost', 'accent'])
      for (const background of ['background', 'surface', 'surfaceMuted', 'raised', 'composer', 'inset'])
        expect(contrast(p[foreground]!, p[background]!), `${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(p.onPrimary!, p.primary!)).toBeGreaterThanOrEqual(7);
  });
}
test('configured launcher and splash assets exist at packaging resolution', () => {
  const config = JSON.parse(readFileSync('apps/mobile/app.json', 'utf8')).expo;
  const files = [[config.icon, 1024], [config.android.adaptiveIcon.foregroundImage, 1024], [config.android.adaptiveIcon.monochromeImage, 1024]] as const;
  for (const [file, size] of files) {
    const png = readFileSync(`apps/mobile/${file}`);
    expect(png.subarray(1, 4).toString()).toBe('PNG');
    expect(png.readUInt32BE(16)).toBe(size); expect(png.readUInt32BE(20)).toBe(size);
  }
  for (const file of ['resources/AppIcon.icns', 'resources/AppIconDev.icns', 'resources/windows/AppIcon.ico', 'assets/icons/ion.svg']) expect(existsSync(file)).toBe(true);
});
