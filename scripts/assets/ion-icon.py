"""Build Ion platform assets from assets/icons/ion.svg. Requires Pillow.

Run: python scripts/assets/ion-icon.py. Provider artwork is never modified.

The mark is the ticket on the rail: a bearing ball (the dot of the i) holding
a torn order ticket (its stem). App icons carry it in ink on lime stock, the
same pairing as the brand tile in the desktop sidebar.
"""
from pathlib import Path
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
MARK = ROOT / "assets/icons/ion.svg"
LIME = "#c2ef68"
INK = "#1b2510"
ORIGIN = "Code-authored Ion mark; assets/icons/ion.svg; scripts/assets/ion-icon.py"


def mark(size, color=INK, scale=1.0):
    factor = size * 4 / 24 * scale
    offset = (size * 4 - 24 * factor) / 2
    image = Image.new("RGBA", (size * 4, size * 4))
    draw = ImageDraw.Draw(image)
    for shape in ET.parse(MARK).getroot():
        if shape.tag.endswith("circle"):
            cx, cy, r = (float(shape.attrib[k]) for k in ("cx", "cy", "r"))
            draw.ellipse(tuple(offset + n * factor for n in (cx - r, cy - r, cx + r, cy + r)),
                         fill=color)
        elif shape.tag.endswith("polygon"):
            points = [float(n) for n in shape.attrib["points"].replace(",", " ").split()]
            draw.polygon([(offset + points[i] * factor, offset + points[i + 1] * factor)
                          for i in range(0, len(points), 2)], fill=color)
    return image.resize((size, size), Image.Resampling.LANCZOS)


def tile(size, margin=0.0, radius=0.0, scale=.72):
    """Lime stock with the ink mark; transparent outside a rounded square."""
    image = Image.new("RGBA", (size, size))
    inset = round(size * margin)
    ImageDraw.Draw(image).rounded_rectangle(
        (inset, inset, size - inset - 1, size - inset - 1),
        radius=round((size - 2 * inset) * radius), fill=LIME)
    artwork = mark(size, scale=scale * (1 - 2 * margin))
    image.alpha_composite(artwork)
    return image


def png(image, path):
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text("Source", ORIGIN)
    image.save(ROOT / path, pnginfo=metadata)


if __name__ == "__main__":
    store = tile(1024).convert("RGB")
    png(store, "apps/mobile/assets/images/icon.png")
    png(store.resize((512, 512), Image.Resampling.LANCZOS), "website/public/app-icon.png")
    png(mark(1024, scale=.55), "apps/mobile/assets/images/adaptive-foreground.png")
    png(mark(1024, color="#ffffff", scale=.55), "apps/mobile/assets/images/adaptive-monochrome.png")
    png(tile(512, radius=.26, scale=.70), "apps/mobile/assets/images/splash-icon.png")
    png(tile(64, radius=.26), "apps/mobile/assets/images/favicon.png")
    desktop = tile(1024, margin=.06, radius=.24)
    desktop.save(ROOT / "resources/windows/AppIcon.ico",
                 sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    mac = tile(1024, margin=.1, radius=.225)
    mac.save(ROOT / "resources/AppIcon.icns")
    dev = mac.copy()
    draw = ImageDraw.Draw(dev)
    draw.rounded_rectangle((600, 720, 900, 880), radius=56, fill=INK)
    draw.text((646, 760), "DEV", fill=LIME, font_size=72, stroke_width=1)
    dev.save(ROOT / "resources/AppIconDev.icns")
    (ROOT / "docs/ion-redesign").mkdir(parents=True, exist_ok=True)
    png(desktop.resize((256, 256), Image.Resampling.LANCZOS), "docs/ion-redesign/icon-preview.png")
    print("Built Ion desktop (macOS/Windows/Linux), Android adaptive/themed, splash and favicon assets from canonical SVG.")
