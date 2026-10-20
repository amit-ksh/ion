"""Build Ion platform assets from assets/icons/ion.svg. Requires Pillow.

Run: python scripts/assets/ion-icon.py. Provider artwork is never modified.
"""
from pathlib import Path
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
MARK = ROOT / "assets/icons/ion.svg"
BACKGROUND = "#181b19"
LIME = "#c2ef68"
ORIGIN = "Code-authored Ion mark; assets/icons/ion.svg; scripts/assets/ion-icon.py"


def mark(size, color=LIME, scale=1.0):
    factor = size * 4 / 24 * scale
    offset = (size * 4 - 24 * factor) / 2
    image = Image.new("RGBA", (size * 4, size * 4))
    draw = ImageDraw.Draw(image)
    for shape in ET.parse(MARK).getroot():
        a = {k: float(v) for k, v in shape.attrib.items()
             if k in ("x", "y", "width", "height", "rx", "cx", "cy", "r", "stroke-width")}
        def box(x, y, w, h):
            return tuple(offset + n * factor for n in (x, y, x + w, y + h))
        if shape.tag.endswith("circle"):
            draw.ellipse(box(a["cx"]-a["r"], a["cy"]-a["r"], a["r"]*2, a["r"]*2), fill=color)
        elif shape.tag.endswith("rect"):
            stroke = a.get("stroke-width", 0)
            draw.rounded_rectangle(
                box(a["x"]-stroke/2, a["y"]-stroke/2, a["width"]+stroke, a["height"]+stroke),
                radius=(a["rx"]+stroke/2)*factor, fill=color)
            if stroke:
                draw.rounded_rectangle(
                    box(a["x"]+stroke/2, a["y"]+stroke/2, a["width"]-stroke, a["height"]-stroke),
                    radius=max(0, a["rx"]-stroke/2)*factor, fill=(0, 0, 0, 0))
    return image.resize((size, size), Image.Resampling.LANCZOS)


def png(image, path):
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text("Source", ORIGIN)
    image.save(ROOT / path, pnginfo=metadata)


if __name__ == "__main__":
    tile = Image.new("RGB", (1024, 1024), BACKGROUND)
    artwork = mark(1024, scale=.76)
    tile.paste(artwork, mask=artwork.getchannel("A"))
    png(tile, "apps/mobile/assets/images/icon.png")
    png(mark(1024, scale=.60), "apps/mobile/assets/images/adaptive-foreground.png")
    png(mark(1024, color="#ffffff", scale=.60), "apps/mobile/assets/images/adaptive-monochrome.png")
    png(mark(256, scale=.90), "apps/mobile/assets/images/splash-icon.png")
    png(tile.resize((64, 64), Image.Resampling.LANCZOS), "apps/mobile/assets/images/favicon.png")
    tile.save(ROOT / "resources/windows/AppIcon.ico", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
    tile.save(ROOT / "resources/AppIcon.icns")
    dev = tile.copy()
    draw = ImageDraw.Draw(dev)
    draw.rounded_rectangle((640, 768, 944, 944), radius=56, fill=LIME)
    draw.text((683, 814), "DEV", fill=BACKGROUND, font_size=72, stroke_width=1)
    dev.save(ROOT / "resources/AppIconDev.icns")
    (ROOT / "docs/ion-redesign").mkdir(parents=True, exist_ok=True)
    png(tile.resize((256,256), Image.Resampling.LANCZOS), "docs/ion-redesign/icon-preview.png")
    print("Built Ion desktop, Android adaptive/themed, splash and favicon assets from canonical SVG.")
