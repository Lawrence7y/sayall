"""Crop the existing RC003 photo's navigation ring, with an antialiased alpha edge.
No image synthesis or external download. Run from any working directory.
"""
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "public" / "RC003-remote-photo@2x.png"
DESTINATION = ROOT / "public" / "remote-navigation-ring.png"


def crop_ring() -> None:
    photo = Image.open(SOURCE).convert("RGBA")
    ring = photo.crop((123, 121, 281, 279))
    # Keep the black hardware rim, remove the silver remote body around it.
    mask = Image.new("L", (158 * 4, 158 * 4), 0)
    ImageDraw.Draw(mask).ellipse((4, 4, 628, 628), fill=255)
    mask = mask.resize(ring.size, Image.Resampling.LANCZOS)
    ring.putalpha(ImageChops.multiply(ring.getchannel("A"), mask))
    assert all(ring.getpixel(point)[3] == 0 for point in [(0, 0), (157, 0), (0, 157), (157, 157)])
    assert ring.getpixel((79, 79))[3] == 255
    ring.save(DESTINATION, optimize=True)
    print(f"ring_crop size={ring.width}x{ring.height} transparent_corners=true")


if __name__ == "__main__":
    crop_ring()
