import asyncio
from playwright.async_api import async_playwright
D='/home/claude/polka/design/'
async def main():
    big=open(D+'icon.svg').read(); small=open(D+'icon-small.svg').read()
    async with async_playwright() as p:
        b=await p.chromium.launch()
        for s in (16,24,32,48,64,128,256,512):
            svg = small if s <= 32 else big
            pg=await b.new_page(viewport={'width':s,'height':s},device_scale_factor=1)
            await pg.set_content(f'<html><body style="margin:0;background:transparent">{svg.replace("<svg ",f"<svg width=\"{s}\" height=\"{s}\" ",1)}</body></html>')
            await pg.screenshot(path=f'{D}icon-{s}.png',omit_background=True)
            await pg.close()
        await b.close()
asyncio.run(main())
