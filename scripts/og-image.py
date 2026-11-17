"""Builds the website's social preview card (site/assets/og-image.png, 1200x630).

Usage: python scripts/og-image.py
Uses the real app icon and banner screenshot; text in Segoe UI (Windows system font).
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
W, H = 1200, 630
BASE = (6, 8, 12)
CYAN = (57, 213, 255)
INK = (243, 246, 250)
MUTED = (160, 174, 190)
FONTS = Path("C:/Windows/Fonts")


def font(name, size):
    return ImageFont.truetype(str(FONTS / name), size)


def glow(img, center, radius, color, alpha):
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    x, y = center
    d.ellipse((x - radius, y - radius, x + radius, y + radius), fill=color + (alpha,))
    layer = layer.filter(ImageFilter.GaussianBlur(radius * 0.45))
    img.alpha_composite(layer)


def rounded(im, r):
    mask = Image.new("L", im.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, im.width - 1, im.height - 1), r, fill=255)
    out = im.convert("RGBA")
    out.putalpha(mask)
    return out


def main():
    card = Image.new("RGBA", (W, H), BASE + (255,))
    glow(card, (1080, 40), 420, CYAN, 70)
    glow(card, (60, 600), 360, (165, 148, 255), 45)

    # Real banner screenshot, right side, with a soft cyan halo and shadow.
    shot = Image.open(ROOT / "site/assets/shots/win-main.webp").convert("RGBA")
    sw = 470
    shot = shot.resize((sw, round(shot.height * sw / shot.width)), Image.LANCZOS)
    shot = rounded(shot, 18)
    sx, sy = W - sw - 70, (H - shot.height) // 2 + 10
    glow(card, (sx + sw // 2, sy + shot.height // 2), 260, CYAN, 55)
    shadow = Image.new("RGBA", card.size, (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle((sx, sy + 24, sx + sw, sy + shot.height + 24), 18, fill=(0, 0, 0, 170))
    card.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(28)))
    card.alpha_composite(shot, (sx, sy))

    d = ImageDraw.Draw(card)
    # Brand row.
    icon = Image.open(ROOT / "src-tauri/icons/icon.png").convert("RGBA").resize((64, 64), Image.LANCZOS)
    card.alpha_composite(rounded(icon, 14), (72, 74))
    d.text((152, 84), "Shuusshy", font=font("seguisb.ttf", 40), fill=INK)

    # Headline.
    big = font("seguisb.ttf", 82)
    d.text((68, 196), "Less noise.", font=big, fill=INK)
    d.text((68, 290), "More voice.", font=big, fill=CYAN)

    # Subline.
    sub = font("segoeui.ttf", 29)
    d.text((72, 410), "Offline microphone noise cancellation", font=sub, fill=MUTED)
    d.text((72, 448), "for Teams, Discord and Zoom.", font=sub, fill=MUTED)

    # Chips: translucent, so they are drawn on their own layer and blended in.
    chip = font("seguisb.ttf", 22)
    chips = Image.new("RGBA", card.size, (0, 0, 0, 0))
    cd = ImageDraw.Draw(chips)
    x = 72
    for label in ("Free", "Windows", "macOS beta", "No account"):
        tw = cd.textlength(label, font=chip)
        cd.rounded_rectangle((x, 520, x + tw + 32, 560), 20, fill=(255, 255, 255, 20), outline=(255, 255, 255, 60))
        x += tw + 44
    card.alpha_composite(chips)
    x = 72
    for label in ("Free", "Windows", "macOS beta", "No account"):
        tw = d.textlength(label, font=chip)
        d.text((x + 16, 525), label, font=chip, fill=INK)
        x += tw + 44

    out = ROOT / "site/assets/og-image.png"
    card.convert("RGB").save(out, optimize=True)
    print(out, out.stat().st_size // 1024, "KB")


if __name__ == "__main__":
    main()
