"""Takes the user guide's screenshots (src/assets/guide/*.webp) from the dev server's demo mode.

Usage (with `npm run dev` running on port 1420):
    python scripts/guide-screenshots.py

Renders each view with headless Microsoft Edge at 2x, crops to the banner (transparent page
background) and saves a WebP. Screens come from the real interface code with sample data
(src/dev/demo.ts); the macOS screens use the macOS wording but are rendered on Windows, so they
use Windows fonts.
"""

import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import quote

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "assets" / "guide"
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
BASE = "http://localhost:1420/"
MAC_UA = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 "
    "(KHTML, like Gecko) Version/17.0 Safari/605.1.15"
)

SHOTS = {
    "main": "",
    "compact": "compact=1",
    "test": "test=1",
    "setup": "state=nocable&view=settings&section=Audio Output",
    "output": "view=settings&section=Audio Output",
    "mic": "view=settings&section=Microphone",
    "noise": "view=settings&section=Noise Suppression",
    "appearance": "view=settings&section=Appearance",
    "startup": "view=settings&section=Startup and Behavior",
    "app": "view=settings&section=Application and Updates",
}


def capture(url: str, mac: bool, png: Path) -> None:
    with tempfile.TemporaryDirectory() as profile:
        args = [
            EDGE,
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--no-first-run",
            f"--user-data-dir={profile}",
            "--force-device-scale-factor=2",
            "--window-size=600,900",
            "--default-background-color=00000000",
            "--virtual-time-budget=6000",
            f"--screenshot={png}",
        ]
        if mac:
            args.append(f"--user-agent={MAC_UA}")
        subprocess.run(args + [url], check=True, capture_output=True, timeout=120)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for os_name, demo in (("win", "win"), ("mac", "mac")):
            for name, params in SHOTS.items():
                query = f"demo={demo}" + (f"&{params}" if params else "")
                url = BASE + "?" + quote(query, safe="=&")
                png = Path(tmp) / f"{os_name}-{name}.png"
                capture(url, os_name == "mac", png)
                img = Image.open(png).convert("RGBA")
                box = img.getchannel("A").point(lambda a: 255 if a > 8 else 0).getbbox()
                if not box:
                    sys.exit(f"empty screenshot: {png.name}")
                img = img.crop(box)
                dest = OUT / f"{os_name}-{name}.webp"
                img.save(dest, "WEBP", quality=82, method=6)
                print(f"{dest.name}: {img.width}x{img.height}, {dest.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
