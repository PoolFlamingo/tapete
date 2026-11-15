//! **El logo de Tapete en los colores del tema**: el mismo SVG del icono de la
//! app (`src-tauri/icons/icono.svg`) con el paño y la carta en los colores que
//! le da la pantalla (los del `<Logo>`: el primario del tema oscurecido en
//! degradado y la carta casi blanca), pintado con resvg. La app lo pone de
//! icono de la ventana y, en Linux, en el tema de iconos del usuario
//! (`crate::icono` de la app).

use image::RgbaImage;

/// El SVG del icono, tal cual (los colores de fábrica, los del tema Nature).
const SVG: &str = include_str!("../../../icons/icono.svg");

/// Los cuatro colores del logo (`[r, g, b]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColoresIcono {
    /// Arriba del degradado del paño.
    pub claro: [u8; 3],
    /// El medio del degradado y el fondo.
    pub medio: [u8; 3],
    /// Abajo del degradado.
    pub oscuro: [u8; 3],
    /// La mano y la carta.
    pub figura: [u8; 3],
}

/// Los del icono de la app instalada (los de `icono.svg`).
pub const DE_FABRICA: ColoresIcono = ColoresIcono {
    claro: [0x1f, 0x7a, 0x45],
    medio: [0x14, 0x53, 0x2d],
    oscuro: [0x0b, 0x3b, 0x1f],
    figura: [0xf0, 0xfd, 0xf4],
};

/// Un `#rrggbb` (sin alfa).
pub fn de_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// El SVG con los colores de `c`.
pub fn svg(c: &ColoresIcono) -> String {
    let f = DE_FABRICA;
    // Primero a marcas y luego a los colores: un color nuevo igual a uno de
    // fábrica no se vuelve a cambiar.
    let pares = [
        (f.claro, c.claro, "\u{1}claro"),
        (f.medio, c.medio, "\u{1}medio"),
        (f.oscuro, c.oscuro, "\u{1}oscuro"),
        (f.figura, c.figura, "\u{1}figura"),
    ];
    let mut s = SVG.to_string();
    for (de, _, marca) in pares {
        s = s.replace(&hex(de), marca);
    }
    for (_, a, marca) in pares {
        s = s.replace(marca, &hex(a));
    }
    s
}

/// El icono de `lado` × `lado` px, sin premultiplicar (como lo quieren el PNG
/// y las ventanas). `None` si el SVG no se entiende (no pasa: es el nuestro).
pub fn pintar(c: &ColoresIcono, lado: u32) -> Option<RgbaImage> {
    let arbol = resvg::usvg::Tree::from_str(&svg(c), &resvg::usvg::Options::default()).ok()?;
    let mut lienzo = tiny_skia::Pixmap::new(lado, lado)?;
    let escala = lado as f32 / arbol.size().width();
    resvg::render(
        &arbol,
        tiny_skia::Transform::from_scale(escala, escala),
        &mut lienzo.as_mut(),
    );
    let mut rgba = Vec::with_capacity((lado * lado * 4) as usize);
    for p in lienzo.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(lado, lado, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn de_fabrica_es_el_svg_de_siempre() {
        assert_eq!(svg(&DE_FABRICA), SVG);
        // Cada color de fábrica está en el SVG (si cambia el icono, esto avisa).
        for c in [
            DE_FABRICA.claro,
            DE_FABRICA.medio,
            DE_FABRICA.oscuro,
            DE_FABRICA.figura,
        ] {
            assert!(SVG.contains(&hex(c)), "{}", hex(c));
        }
    }

    #[test]
    fn pinta_con_los_colores_pedidos() {
        let naranja = ColoresIcono {
            claro: [0xc2, 0x41, 0x0c],
            medio: [0x9a, 0x34, 0x12],
            oscuro: [0x7c, 0x2d, 0x12],
            figura: [0xff, 0xf7, 0xed],
        };
        let s = svg(&naranja);
        assert!(!s.contains("#14532d") && s.contains("#9a3412") && s.contains("#fff7ed"));
        let img = pintar(&naranja, 64).unwrap();
        assert_eq!(img.dimensions(), (64, 64));
        // En la esquina de abajo, el paño (opaco y naranja, no verde).
        let p = img.get_pixel(8, 56);
        assert_eq!(p[3], 255);
        assert!(p[0] > p[1], "{p:?}");
    }

    #[test]
    fn hex_valido_e_invalido() {
        assert_eq!(de_hex("#1f7a45"), Some([0x1f, 0x7a, 0x45]));
        assert_eq!(de_hex("1f7a45"), None);
        assert_eq!(de_hex("#1f7a4"), None);
        assert_eq!(de_hex("#zz7a45"), None);
    }
}
