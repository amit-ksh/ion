"""Rasterize ion's code-authored geometric mark for platform packaging."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
size = 1024
image = Image.new("RGB", (size, size))
pixels = image.load()
for y in range(size):
    for x in range(size):
        glow = max(0, 1 - ((x - 760) ** 2 + (y - 150) ** 2) ** .5 / 1050)
        pixels[x, y] = (int(15 + 22 * glow), int(29 + 38 * glow), int(21 + 10 * glow))
draw = ImageDraw.Draw(image)
green = (194, 248, 123)
draw.rounded_rectangle((260, 370, 420, 780), radius=70, fill=green)
draw.ellipse((260, 180, 420, 340), fill=green)
draw.rounded_rectangle((478, 370, 760, 780), radius=140, outline=green, width=92)
image.save(ROOT / "apps/mobile/assets/images/icon.png")
image.save(ROOT / "resources/windows/AppIcon.ico", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
image.save(ROOT / "resources/AppIcon.icns")
image.save(ROOT / "resources/AppIconDev.icns")
image.resize((256,256)).save(ROOT / "apps/mobile/assets/images/splash-icon.png")
image.resize((64,64)).save(ROOT / "apps/mobile/assets/images/favicon.png")
mark = Image.new("RGBA", (96,96), (0,0,0,0))
ImageDraw.Draw(mark).polygon([(48,8),(59,37),(88,48),(59,59),(48,88),(37,59),(8,48),(37,37)], fill=(0,0,0,255))
mark.save(ROOT / "apps/mobile/assets/images/providers/antigravity.png")
