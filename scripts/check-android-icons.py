"""Verify actual Android launcher pixels against canonical Expo artwork.

Run after prebuild, and again on the APK: python scripts/check-android-icons.py [app.apk]
Requires Pillow. Does not trust file names or app.json alone.
"""
import io
import json
import os
import re
import subprocess
import sys
import zipfile
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageStat

ROOT = Path(__file__).resolve().parents[1]
MOBILE = ROOT / 'apps/mobile'
config = json.loads((MOBILE / 'app.json').read_text(encoding='utf-8'))['expo']
archive = zipfile.ZipFile(sys.argv[1]) if len(sys.argv) > 1 else None
packaged = {}
if archive:
    sdk = Path(os.environ.get('ANDROID_HOME', ROOT / '.waku-cache/android-sdk'))
    candidates = sorted((sdk / 'build-tools').glob('*/aapt2.exe' if os.name == 'nt' else '*/aapt2'))
    if not candidates:
        raise SystemExit('Set ANDROID_HOME to an SDK containing aapt2 to verify APK resources.')
    dump = subprocess.check_output([str(candidates[-1]), 'dump', 'resources', sys.argv[1]], text=True, encoding='utf-8')
    name = None
    for line in dump.splitlines():
        resource = re.search(r'resource 0x\w+ mipmap/(ic_launcher\w*)$', line)
        if resource:
            name = resource.group(1)
        elif 'resource 0x' in line or 'type ' in line and '(file)' not in line:
            name = None
        entry = re.search(r'\((\w+dpi)\) \(file\) (\S+)', line)
        if name and entry:
            packaged[name, entry.group(1)] = entry.group(2)
res = MOBILE / 'android/app/src/main/res'
checks = {
    'ic_launcher': config['icon'],
    'ic_launcher_round': config['icon'],
    'ic_launcher_foreground': config['android']['adaptiveIcon']['foregroundImage'],
    'ic_launcher_monochrome': config['android']['adaptiveIcon']['monochromeImage'],
}
failures = []
for folder in ['mdpi', 'hdpi', 'xhdpi', 'xxhdpi', 'xxxhdpi']:
    for name, source in checks.items():
        relative = f'res/mipmap-{folder}/{name}.webp'
        data = archive.read(packaged[name, folder]) if archive else (res / f'mipmap-{folder}/{name}.webp').read_bytes()
        actual = Image.open(io.BytesIO(data)).convert('RGBA')
        expected = Image.open(MOBILE / source).convert('RGBA').resize(actual.size, Image.Resampling.LANCZOS)
        if name == 'ic_launcher_round':
            # Legacy round icons are clipped by the encoder; compare inside that circle.
            mask = Image.new('L', actual.size)
            inset = max(2, round(actual.width * .04))
            ImageDraw.Draw(mask).ellipse((inset, inset, actual.width - inset, actual.height - inset), fill=255)
            blank = Image.new('RGBA', actual.size)
            actual = Image.composite(actual, blank, mask)
            expected = Image.composite(expected, blank, mask)
        # WebP and Sharp/Pillow resampling differ slightly at antialiased edges.
        error = sum(ImageStat.Stat(ImageChops.difference(actual, expected)).mean) / 4
        if error > 5:
            failures.append(f'{relative}: pixel error {error:.2f} (expected <= 5)')
if failures:
    raise SystemExit('Launcher artwork is stale:\n' + '\n'.join(failures))
print('PASS: all 20 launcher, round, adaptive and themed icon resources match current Ion artwork.')
