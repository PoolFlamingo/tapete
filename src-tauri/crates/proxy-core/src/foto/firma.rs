//! **Huella de la ilustración**: para saber qué impresión de una carta es la
//! de la foto cuando el pie no lo dice (cartas anteriores a 2014, o un pie que
//! el OCR no lee), se compara su ilustración con las de cada impresión de
//! Scryfall.
//!
//! Dos partes, cada una robusta a lo que cambia de una foto a un escaneo:
//! - un dHash de 16 × 16 (si cada zona es más clara que la de su derecha):
//!   aguanta brillo, contraste, JPEG y un poco de desenfoque;
//! - el color medio de una rejilla de 6 × 4 en Lab, con la luminosidad
//!   centrada (la luz de la habitación cambia el brillo, no tanto el tono).

use image::RgbaImage;

use super::medias;

/// Dónde va la ilustración, en fracciones de la carta (x0, y0, x1, y1). Vale
/// para el marco moderno y el antiguo; los marcos sin ventana (extendidos,
/// a sangre) también tienen la ilustración ahí.
pub const ARTE: (f32, f32, f32, f32) = (0.08, 0.11, 0.92, 0.55);

/// La carta entera: para distinguir, entre las impresiones con la misma
/// ilustración, la de marco distinto (M10 frente a 2X2, extendida, sin borde).
pub const ENTERA: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 1.0);

const COLUMNAS: usize = 6;
const FILAS: usize = 4;

/// A partir de qué diferencia media de color (ΔE) dos ilustraciones ya no
/// se parecen nada.
const DELTA_E_MAXIMO: f32 = 40.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Firma {
    hash: [u64; 4],
    color: Vec<[f32; 3]>,
}

impl Firma {
    /// La huella de la ilustración de una carta (plana y derecha, de
    /// cualquier medida).
    pub fn de(carta: &RgbaImage) -> Firma {
        Firma::de_zona(carta, ARTE)
    }

    /// La huella de una zona de la carta (`x0, y0, x1, y1` en fracciones).
    pub fn de_zona(carta: &RgbaImage, zona: (f32, f32, f32, f32)) -> Firma {
        let (w, h) = (carta.width() as f32, carta.height() as f32);
        let x0 = ((zona.0 * w) as u32).min(carta.width().saturating_sub(1));
        let y0 = ((zona.1 * h) as u32).min(carta.height().saturating_sub(1));
        let caja = (
            x0,
            y0,
            ((zona.2 * w) as u32).clamp(x0 + 1, carta.width().max(x0 + 1)),
            ((zona.3 * h) as u32).clamp(y0 + 1, carta.height().max(y0 + 1)),
        );
        Firma {
            hash: dhash(&medias(carta, caja, 17, 16)),
            color: lab_centrado(&medias(carta, caja, COLUMNAS, FILAS)),
        }
    }

    /// Lo distintas que son dos huellas: 0 = iguales, 1 = nada que ver.
    pub fn distancia(&self, otra: &Firma) -> f32 {
        let bits: u32 = self
            .hash
            .iter()
            .zip(&otra.hash)
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();
        let hamming = bits as f32 / 256.0;
        let de: f32 = self
            .color
            .iter()
            .zip(&otra.color)
            .map(|(a, b)| {
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
            })
            .sum::<f32>()
            / self.color.len().max(1) as f32;
        0.5 * hamming + 0.5 * (de / DELTA_E_MAXIMO).min(1.0)
    }
}

/// dHash de una rejilla de 17 × 16 colores: un bit por zona, si es más clara
/// que la de su derecha.
fn dhash(rejilla: &[[f32; 3]]) -> [u64; 4] {
    let gris: Vec<f32> = rejilla
        .iter()
        .map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2])
        .collect();
    let mut hash = [0u64; 4];
    for y in 0..16 {
        for x in 0..16 {
            if gris[y * 17 + x + 1] > gris[y * 17 + x] {
                let bit = y * 16 + x;
                hash[bit / 64] |= 1 << (bit % 64);
            }
        }
    }
    hash
}

/// sRGB (0–255) a CIE Lab (D65).
fn lab(c: [f32; 3]) -> [f32; 3] {
    let lineal = |v: f32| {
        let v = v / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lineal(c[0]), lineal(c[1]), lineal(c[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.950_47;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.088_83;
    let f = |t: f32| {
        if t > 0.008_856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Los colores en Lab, con la luminosidad media restada.
fn lab_centrado(rejilla: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut v: Vec<[f32; 3]> = rejilla.iter().map(|c| lab(*c)).collect();
    let media = v.iter().map(|c| c[0]).sum::<f32>() / v.len().max(1) as f32;
    for c in &mut v {
        c[0] -= media;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::super::{pruebas, ALTO, ANCHO};
    use super::*;
    use crate::imaging::{decode::decode, encode::jpeg};

    /// La misma carta como saldría de una foto: más clara, desenfocada y en
    /// un JPEG malo.
    fn como_foto(carta: &RgbaImage) -> RgbaImage {
        let mut c = carta.clone();
        for p in c.pixels_mut() {
            for k in 0..3 {
                p[k] = (p[k] as f32 * 1.2).min(255.0) as u8;
            }
        }
        let c = image::imageops::blur(&c, 1.0);
        decode(&jpeg(&c, 60).expect("jpeg")).expect("decode")
    }

    #[test]
    fn la_misma_ilustracion_se_parece_mas_que_otra() {
        let a = pruebas::carta(1);
        let foto = Firma::de(&como_foto(&a));
        let fa = Firma::de(&a);
        for otra in 2..6 {
            let fb = Firma::de(&pruebas::carta(otra));
            assert!(
                foto.distancia(&fa) < foto.distancia(&fb),
                "semilla {otra}: {} frente a {}",
                foto.distancia(&fa),
                foto.distancia(&fb)
            );
        }
    }

    #[test]
    fn la_medida_no_importa() {
        // Scryfall «small» es de 146 × 204; la foto enderezada, de 630 × 880.
        let a = pruebas::carta(3);
        let pequena = image::imageops::resize(&a, 146, 204, image::imageops::FilterType::Triangle);
        let d = Firma::de(&a).distancia(&Firma::de(&pequena));
        assert!(d < 0.12, "{d}");
        assert_eq!((a.width(), a.height()), (ANCHO, ALTO));
    }

    #[test]
    fn la_carta_entera_distingue_el_marco_con_la_misma_ilustracion() {
        let a = pruebas::carta(4);
        // La misma ilustración con otro marco (más oscuro y frío).
        let mut b = a.clone();
        for (x, y, p) in b.enumerate_pixels_mut() {
            let (u, v) = (x as f32 / ANCHO as f32, y as f32 / ALTO as f32);
            let en_arte = (ARTE.0..ARTE.2).contains(&u) && (ARTE.1..ARTE.3).contains(&v);
            if !en_arte {
                *p = image::Rgba([60, 70, 110, 255]);
            }
        }
        let foto = como_foto(&a);
        let arte = |c: &RgbaImage| Firma::de(&foto).distancia(&Firma::de(c));
        let entera =
            |c: &RgbaImage| Firma::de_zona(&foto, ENTERA).distancia(&Firma::de_zona(c, ENTERA));
        assert!((arte(&a) - arte(&b)).abs() < 0.05, "el arte es el mismo");
        assert!(
            entera(&a) < entera(&b),
            "{} frente a {}",
            entera(&a),
            entera(&b)
        );
    }

    #[test]
    fn una_huella_no_dista_de_si_misma() {
        let f = Firma::de(&pruebas::carta(7));
        assert_eq!(f.distancia(&f), 0.0);
    }

    #[test]
    fn el_blanco_en_lab() {
        let l = lab([255.0, 255.0, 255.0]);
        assert!(
            (l[0] - 100.0).abs() < 0.5 && l[1].abs() < 0.5 && l[2].abs() < 0.5,
            "{l:?}"
        );
    }
}
