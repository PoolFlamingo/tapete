"""Termina el icono de la app después de `tauri icon`.

El logo es el de la web (`src-tauri/icons/icono.svg`: la mano que juega una carta,
`game-icons:card-play`, CC BY 3.0, sobre el paño verde). Se rehace a mano cuando cambie:

    (cd src-tauri && cargo run -p proxy-core --example icono)
    pnpm tauri icon src-tauri/icons/icono-1024.png
    python3 scripts/icono.py

`tauri icon` pone en el primer plano del icono adaptable de Android el icono entero
(con su cuadrado redondeado) sobre fondo blanco. Aquí el primer plano pasa a ser solo
el dibujo (`icono-frente.svg`, dentro de la zona segura de 66 dp) y el fondo, el verde
del paño. También quita `icons/ios/`, que la app no usa.
"""
import re
import shutil
from pathlib import Path

from PIL import Image

RAIZ = Path(__file__).resolve().parent.parent
RES = RAIZ / "src-tauri/gen/android/app/src/main/res"
VERDE = "#14532D"
# Lado del primer plano adaptable (108 dp) en cada densidad.
LADOS = {"mdpi": 108, "hdpi": 162, "xhdpi": 216, "xxhdpi": 324, "xxxhdpi": 432}

frente = Image.open(RAIZ / "src-tauri/icons/icono-frente-1024.png").convert("RGBA")
for densidad, lado in LADOS.items():
    frente.resize((lado, lado), Image.LANCZOS).save(
        RES / f"mipmap-{densidad}/ic_launcher_foreground.png"
    )

fondo = RES / "values/ic_launcher_background.xml"
texto = fondo.read_text(encoding="utf-8")
fondo.write_text(
    re.sub(
        r'(<color name="ic_launcher_background">)#[0-9A-Fa-f]+(</color>)',
        rf"\g<1>{VERDE}\2",
        texto,
    ),
    encoding="utf-8",
)

shutil.rmtree(RAIZ / "src-tauri/icons/ios", ignore_errors=True)
print("Icono adaptable de Android: primer plano y fondo puestos.")
