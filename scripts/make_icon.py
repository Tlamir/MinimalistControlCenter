"""Regenerate the original vector-style Windows icon; Pillow is a development dependency only."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent.parent
im = Image.new('RGBA', (1024, 1024))
d = ImageDraw.Draw(im)
d.rounded_rectangle((0, 0, 1023, 1023), radius=224, fill='#18724c')
for x, y in [(256, 384), (512, 640), (768, 384)]:
    d.rounded_rectangle((x-32, 224, x+32, 800), radius=32, fill='white')
    d.rounded_rectangle((x-96, y-64, x+96, y+64), radius=64, fill='#b6e7cc')
im.save(root / 'assets/app.ico', sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])
