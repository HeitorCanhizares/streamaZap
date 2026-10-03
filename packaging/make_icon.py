"""Gera os ícones do app a partir da arte original.

Remove o fundo branco fora do quadrado arredondado (só o branco ligado aos
cantos, para não apagar o branco do texto) e exporta:
  * streamazap/assets/icon.png  (ícone das janelas)
  * packaging/icon.ico          (executável e instalador)

Uso: python packaging/make_icon.py [caminho/da/arte]
Sem argumento, usa packaging/icon-source.* (png, jpg ou webp). Para trocar o
ícone basta substituir esse arquivo: a build do GitHub regenera os ícones.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
MARKER = (255, 0, 255)


def cut_background(image: Image.Image) -> Image.Image:
    rgb = image.convert("RGB")
    filled = rgb.copy()
    w, h = filled.size
    for corner in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]:
        if filled.getpixel(corner) != MARKER:
            ImageDraw.floodfill(filled, corner, MARKER, thresh=60)
    # Fundo = pixels marcados; alarga 2 px para engolir a franja clara da borda.
    mask = Image.new("L", (w, h), 0)
    src, out = filled.load(), mask.load()
    for y in range(h):
        for x in range(w):
            if src[x, y] == MARKER:
                out[x, y] = 255
    mask = mask.filter(ImageFilter.MaxFilter(5))
    alpha = Image.eval(mask, lambda v: 255 - v).filter(ImageFilter.GaussianBlur(1.2))
    result = rgb.convert("RGBA")
    result.putalpha(alpha)
    return result.crop(result.getbbox())


def main(source: str) -> None:
    icon = cut_background(Image.open(source))
    side = max(icon.size)
    square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    square.paste(icon, ((side - icon.width) // 2, (side - icon.height) // 2))
    square.resize((512, 512), Image.LANCZOS).save(ROOT / "streamazap" / "assets" / "icon.png", optimize=True)
    square.resize((256, 256), Image.LANCZOS).save(
        ROOT / "packaging" / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    )
    print("ícones gerados")


def find_source() -> str:
    candidates = sorted((ROOT / "packaging").glob("icon-source.*"))
    if not candidates:
        sys.exit("packaging/icon-source.* não encontrado")
    return str(candidates[0])


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else find_source())
