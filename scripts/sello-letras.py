#!/usr/bin/env python3
"""Las letras recortadas del sello en japonés, coreano y chino (plan Tapete, Fase B).

El sello («Tapete · 非売品») va en el idioma de la carta, y Montserrat no tiene ideogramas
ni hangul. Las Noto Sans JP/KR/SC/TC enteras pesan de 10 a 18 MB (la app las baja aparte,
`letras_cjk.rs`); aquí se recortan a los caracteres de los textos del sello de cada idioma
(`proxy-core/assets/sello/textos.json`) y se dejan a peso 600: unos pocos KB cada una, que
van incrustadas en la app para que el sello salga siempre, haya bajado las letras CJK o no.

Las de origen son las mismas que `src-tauri/assets/letras-cjk.json` (commit fijo de
google/fonts y SHA-256 de cada fichero): el resultado es reproducible.

Uso (necesita fontTools; por ejemplo en un entorno aparte):
    python3 -m venv /tmp/ft && /tmp/ft/bin/pip install fonttools==4.55.3
    /tmp/ft/bin/python -I scripts/sello-letras.py <carpeta para las descargas>

Si se cambia un texto en ja, ko, zhs o zht de `textos.json`, hay que volver a pasarlo.
"""

import hashlib
import json
import pathlib
import sys
import urllib.request

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

RAIZ = pathlib.Path(__file__).resolve().parent.parent
CATALOGO = RAIZ / "src-tauri" / "assets" / "letras-cjk.json"
TEXTOS = RAIZ / "src-tauri" / "crates" / "proxy-core" / "assets" / "sello" / "textos.json"
SALIDA = TEXTOS.parent
REPO = "https://raw.githubusercontent.com/google/fonts"
PESO = 600


def bajar(url: str, sha: str, destino: pathlib.Path) -> bytes:
    if destino.exists() and hashlib.sha256(destino.read_bytes()).hexdigest() == sha:
        return destino.read_bytes()
    with urllib.request.urlopen(url, timeout=120) as r:
        datos = r.read()
    real = hashlib.sha256(datos).hexdigest()
    if real != sha:
        sys.exit(f"{url}: SHA-256 {real}, se esperaba {sha}")
    destino.write_bytes(datos)
    return datos


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    descargas = pathlib.Path(sys.argv[1])
    descargas.mkdir(parents=True, exist_ok=True)
    catalogo = json.loads(CATALOGO.read_text())
    textos = json.loads(TEXTOS.read_text())
    base = f"{REPO}/{catalogo['commit']}/{catalogo['base']}"

    for letra in catalogo["letras"]:
        lang = letra["lang"]
        origen = descargas / pathlib.Path(letra["ruta"]).name
        bajar(base + letra["ruta"], letra["sha256"], origen)
        fuente = TTFont(origen)
        fuente = instancer.instantiateVariableFont(fuente, {"wght": PESO})
        caracteres = set(textos["carta"][lang]) | set(textos["hoja"][lang])
        opciones = subset.Options()
        opciones.layout_features = ["*"]
        opciones.name_IDs = ["*"]
        opciones.notdef_outline = True
        opciones.hinting = False
        recorte = subset.Subsetter(opciones)
        recorte.populate(unicodes=sorted(ord(c) for c in caracteres))
        recorte.subset(fuente)
        destino = SALIDA / f"sello-{lang}.ttf"
        fuente.save(destino)
        print(f"{destino.name}: {destino.stat().st_size} bytes, {len(caracteres)} caracteres")

    lic = catalogo["licencia"]
    datos = bajar(base + lic["ruta"], lic["sha256"], descargas / "OFL.txt")
    (SALIDA / "Noto-CJK-LICENSE.txt").write_bytes(datos)


if __name__ == "__main__":
    main()
