"""Проверочная библиотека комиксов для разработки: ч/б «манга» с цветной вклейкой и цветной комикс.
python3 dev/make_demo.py /tmp/polka-demo"""
import io, math, random, sys, zipfile
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageFont

FONT = next((p for p in ['/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'] if Path(p).exists()), None)
LINES = ['Ты опоздал.', 'Город не спит\nпосле полуночи.', 'Где карта?', 'Держись!', 'Это только\nначало…', 'Слышишь?\nОни идут.']


def font(s): return ImageFont.truetype(FONT, s) if FONT else ImageFont.load_default()


def tone(w, h, step, r):
    t = Image.new('L', (w, h), 255); d = ImageDraw.Draw(t)
    for y in range(0, h, step):
        for x in range((y // step % 2) * step // 2, w, step): d.ellipse([x - r, y - r, x + r, y + r], fill=40)
    return t


def figure(d, cx, cy, s, ink, fill):
    d.ellipse([cx - s, cy - s, cx + s, cy + s], fill=fill, outline=ink, width=6)                 # голова
    d.polygon([(cx - s * 1.1, cy - s * .2), (cx - s * .2, cy - s * 1.5), (cx + s * .6, cy - s * 1.2), (cx + s * 1.2, cy - s * .1), (cx, cy - s * .7)], fill=ink)  # волосы
    d.ellipse([cx - s * .45, cy - s * .05, cx - s * .2, cy + s * .25], fill=ink); d.ellipse([cx + s * .2, cy - s * .05, cx + s * .45, cy + s * .25], fill=ink)
    d.arc([cx - s * .35, cy + s * .25, cx + s * .35, cy + s * .65], 20, 160, fill=ink, width=5)
    d.polygon([(cx - s * 1.6, cy + s * 3.2), (cx - s * .9, cy + s * 1.1), (cx + s * .9, cy + s * 1.1), (cx + s * 1.6, cy + s * 3.2)], fill=fill, outline=ink)


def bubble(d, box, text, ink):
    x0, y0, x1, y1 = box
    d.ellipse(box, fill='white', outline=ink, width=5)
    d.polygon([((x0 + x1) / 2 - 20, y1 - 12), ((x0 + x1) / 2 + 30, y1 + 50), ((x0 + x1) / 2 + 24, y1 - 14)], fill='white', outline=ink)
    d.multiline_text(((x0 + x1) / 2, (y0 + y1) / 2), text, font=font(38), fill=ink, anchor='mm', align='center', spacing=8)


def page(seed, color=False, W=1400, H=2000):
    rnd = random.Random(seed)
    img = Image.new('RGB', (W, H), 'white'); d = ImageDraw.Draw(img)
    M, G = 70, 28
    rows = [(M, 720), (720 + G, 1290), (1290 + G, H - M)]
    pal = [(214, 86, 60), (60, 118, 182), (236, 190, 70), (88, 160, 110), (150, 92, 170)]
    for ri, (y0, y1) in enumerate(rows):
        cut = rnd.randint(int(W * .38), int(W * .62)) if ri != 1 else None
        panels = [(M, y0, cut - G // 2, y1), (cut + G // 2, y0, W - M, y1)] if cut else [(M, y0, W - M, y1)]
        for pi, (x0, a, x1, b) in enumerate(panels):
            pw, ph = x1 - x0, b - a
            if color:
                c1, c2 = rnd.sample(pal, 2)
                bg = Image.new('RGB', (pw, ph)); bd = ImageDraw.Draw(bg)
                for yy in range(ph): t = yy / ph; bd.line([(0, yy), (pw, yy)], fill=tuple(int(c1[k] * (1 - t) + c2[k] * t) for k in range(3)))
                img.paste(bg, (x0, a))
            else:
                t = tone(pw, ph, rnd.choice([10, 12, 14]), rnd.choice([2, 3, 4]))
                mask = Image.linear_gradient('L').resize((pw, ph)).point(lambda v: 255 - v if pi % 2 else v)
                img.paste(Image.merge('RGB', [t] * 3), (x0, a), mask)
            if rnd.random() < .45:   # линии скорости
                cx, cy = x0 + pw / 2, a + ph / 2
                for k in range(70):
                    ang = rnd.random() * math.tau; r0 = rnd.uniform(.25, .4) * min(pw, ph)
                    d.line([(cx + math.cos(ang) * r0, cy + math.sin(ang) * r0), (cx + math.cos(ang) * pw, cy + math.sin(ang) * pw)], fill=(30, 30, 30), width=2)
            s = min(pw, ph) * rnd.uniform(.13, .18)
            figure(d, x0 + pw * rnd.uniform(.3, .7), a + ph * .45, s, (20, 20, 20), rnd.choice(pal) if color else (235, 235, 235))
            if rnd.random() < .8:
                bw, bh = min(pw * .55, 420), 190
                bx = x0 + 30 if pi % 2 == 0 else x1 - bw - 30
                bubble(d, (bx, a + 30, bx + bw, a + 30 + bh), rnd.choice(LINES), (20, 20, 20))
            d.rectangle([x0, a, x1, b], outline=(10, 10, 10), width=7)
    img = img.filter(ImageFilter.GaussianBlur(.6))
    # скан: желтоватая бумага и шум, как у настоящих сканов
    if not color:
        noise = Image.effect_noise((W, H), 14).convert('RGB')
        img = Image.blend(img, noise, .05)
        img = Image.blend(img, Image.new('RGB', (W, H), (246, 240, 226)), .12)
    return img


def jpg(im, q=95):
    b = io.BytesIO(); im.save(b, 'JPEG', quality=q, subsampling=0); return b.getvalue()


def cbz(path, pages):
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_STORED) as z:
        for i, p in enumerate(pages): z.writestr(f'{i + 1:03d}.jpg', p)


out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/polka-demo')
for v in (1, 2, 3):
    pages = [jpg(page(v * 100 + i, color=(v == 1 and i == 0))) for i in range(10 if v == 1 else 6)]
    cbz(out / 'Ночной город' / f'Nochnoy_gorod_v{v:02d}_[scan].cbz', pages)
cbz(out / 'Комиксы' / 'Hero 001.cbz', [jpg(page(900 + i, color=True), 92) for i in range(5)])
print('ok', out)
