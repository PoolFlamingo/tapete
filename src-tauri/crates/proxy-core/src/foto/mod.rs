//! **Una carta real en una foto**: encontrarla (`detectar`), dejarla plana y
//! derecha (`enderezar`) y sacar una huella de su ilustración para saber qué
//! impresión es (`firma`). Lo usa «Identificar por foto»: la app lee luego el
//! título y el pie con el OCR y busca la carta en Scryfall.
//!
//! Todo es puro y determinista (sin red ni azar), como el resto del crate.

pub mod detectar;
pub mod enderezar;
pub mod firma;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use detectar::detectar;
pub use enderezar::{enderezar, girar180, ALTO, ANCHO};
pub use firma::Firma;

/// Proporción de una carta de Magic (63 × 88 mm): el lado corto entre el largo.
pub const PROPORCION: f32 = 63.0 / 88.0;

/// Un punto de la foto, en píxeles de la imagen original (0, 0 = arriba a la
/// izquierda).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Punto {
    pub x: f32,
    pub y: f32,
}

impl Punto {
    pub fn distancia(self, o: Punto) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

/// Las cuatro esquinas de la carta, en el sentido de las agujas del reloj:
/// arriba a la izquierda, arriba a la derecha, abajo a la derecha y abajo a la
/// izquierda (de la carta, no de la foto).
pub type Esquinas = [Punto; 4];

/// Un plano de grises (0–255), fila a fila.
#[derive(Debug, Clone)]
pub(crate) struct Gris {
    pub ancho: usize,
    pub alto: usize,
    pub px: Vec<f32>,
}

impl Gris {
    pub fn en(&self, x: usize, y: usize) -> f32 {
        self.px[y * self.ancho + x]
    }
}

/// La luminancia de un píxel (Rec. 601, la de siempre en visión).
fn luma(p: &image::Rgba<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Los límites `[desde, hasta)` del bloque `i` de `n` en que se parte `total`.
fn bloque(i: usize, n: usize, total: usize) -> (usize, usize) {
    let a = i * total / n;
    let b = ((i + 1) * total / n).max(a + 1).min(total);
    (a.min(total.saturating_sub(1)), b)
}

/// La foto en grises con el lado mayor en `lado_max` como mucho (media de
/// cada bloque: sin aliasing, y rápido aunque la foto sea de 12 MP). Da
/// también cuántos píxeles de la original hay por píxel reducido (x, y).
pub(crate) fn gris_reducido(img: &RgbaImage, lado_max: u32) -> (Gris, f32, f32) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let factor = (w.max(h) as f32 / lado_max as f32).max(1.0);
    let dw = ((w as f32 / factor).round() as usize).clamp(1, w.max(1));
    let dh = ((h as f32 / factor).round() as usize).clamp(1, h.max(1));
    let mut px = vec![0.0f32; dw * dh];
    for dy in 0..dh {
        let (y0, y1) = bloque(dy, dh, h);
        for dx in 0..dw {
            let (x0, x1) = bloque(dx, dw, w);
            let mut suma = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    suma += luma(img.get_pixel(x as u32, y as u32));
                }
            }
            px[dy * dw + dx] = suma / ((y1 - y0) * (x1 - x0)) as f32;
        }
    }
    let g = Gris {
        ancho: dw,
        alto: dh,
        px,
    };
    (g, w as f32 / dw as f32, h as f32 / dh as f32)
}

/// La media de color (RGB, 0–255) de cada celda de una rejilla de
/// `columnas × filas` sobre el rectángulo `[x0, x1) × [y0, y1)` de la imagen.
pub(crate) fn medias(
    img: &RgbaImage,
    (x0, y0, x1, y1): (u32, u32, u32, u32),
    columnas: usize,
    filas: usize,
) -> Vec<[f32; 3]> {
    let (ancho, alto) = ((x1 - x0).max(1) as usize, (y1 - y0).max(1) as usize);
    let mut v = Vec::with_capacity(columnas * filas);
    for f in 0..filas {
        let (ya, yb) = bloque(f, filas, alto);
        for c in 0..columnas {
            let (xa, xb) = bloque(c, columnas, ancho);
            let mut s = [0.0f32; 3];
            for y in ya..yb {
                for x in xa..xb {
                    let p = img.get_pixel(x0 + x as u32, y0 + y as u32);
                    s[0] += p[0] as f32;
                    s[1] += p[1] as f32;
                    s[2] += p[2] as f32;
                }
            }
            let n = ((yb - ya) * (xb - xa)).max(1) as f32;
            v.push([s[0] / n, s[1] / n, s[2] / n]);
        }
    }
    v
}

/// Lo nítida que es una imagen: la varianza del laplaciano de sus grises,
/// reducidos a 480 px. Una foto movida o desenfocada da poco; el escáner en
/// vivo no lee hasta que pasa de un umbral.
pub fn nitidez(img: &RgbaImage) -> f32 {
    let (g, _, _) = gris_reducido(img, 480);
    if g.ancho < 3 || g.alto < 3 {
        return 0.0;
    }
    let mut valores = Vec::with_capacity((g.ancho - 2) * (g.alto - 2));
    for y in 1..g.alto - 1 {
        for x in 1..g.ancho - 1 {
            let l = g.en(x - 1, y) + g.en(x + 1, y) + g.en(x, y - 1) + g.en(x, y + 1)
                - 4.0 * g.en(x, y);
            valores.push(l);
        }
    }
    let n = valores.len() as f32;
    let media = valores.iter().sum::<f32>() / n;
    valores.iter().map(|v| (v - media).powi(2)).sum::<f32>() / n
}

#[cfg(test)]
pub(crate) mod pruebas {
    //! Fotos inventadas para los tests: una carta (con su ilustración) pegada
    //! sobre un fondo con ruido, en la posición que se quiera.

    use image::{Rgba, RgbaImage};

    use super::enderezar::Homografia;
    use super::{Punto, ALTO, ANCHO};

    /// Generador de números pseudoaleatorios fijo (sin dependencias).
    pub struct Lcg(pub u64);

    impl Lcg {
        pub fn sig(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as u32
        }
    }

    /// Una ilustración suave e inventada (`semilla` la cambia).
    pub fn ilustracion(semilla: u32, x: f32, y: f32) -> [f32; 3] {
        let s = semilla as f32;
        let a = ((x * (3.0 + s * 0.7)).sin() * (y * (2.0 + s * 0.3)).cos() + 1.0) * 0.5;
        let b = ((x * (1.5 + s * 0.2) + y * (4.0 - s * 0.4)).sin() + 1.0) * 0.5;
        [
            60.0 + 180.0 * a,
            40.0 + 160.0 * b,
            50.0 + 150.0 * (1.0 - a * b),
        ]
    }

    /// Una carta plana de `ANCHO × ALTO`: borde negro, marco claro, una
    /// ilustración (de 0,11 a 0,55 de alto) y una caja de texto.
    pub fn carta(semilla: u32) -> RgbaImage {
        RgbaImage::from_fn(ANCHO, ALTO, |x, y| {
            let (u, v) = (x as f32 / ANCHO as f32, y as f32 / ALTO as f32);
            let c = if !(0.04..=0.96).contains(&u) || !(0.03..=0.97).contains(&v) {
                [15.0, 15.0, 15.0]
            } else if (0.08..0.92).contains(&u) && (0.11..0.55).contains(&v) {
                ilustracion(semilla, u * 6.0, v * 6.0)
            } else if (0.08..0.92).contains(&u) && (0.6..0.9).contains(&v) {
                [235.0, 228.0, 210.0]
            } else {
                [200.0, 190.0, 170.0]
            };
            Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255])
        })
    }

    /// Pega `carta` en una foto de `w × h` con fondo de ruido oscuro, con sus
    /// esquinas (TL, TR, BR, BL) en `esquinas`.
    pub fn foto(carta: &RgbaImage, w: u32, h: u32, esquinas: [Punto; 4]) -> RgbaImage {
        // De la foto a la carta: la inversa de la que endereza.
        let destino = [
            Punto { x: 0.0, y: 0.0 },
            Punto {
                x: carta.width() as f32,
                y: 0.0,
            },
            Punto {
                x: carta.width() as f32,
                y: carta.height() as f32,
            },
            Punto {
                x: 0.0,
                y: carta.height() as f32,
            },
        ];
        let hacia_carta = Homografia::de(&esquinas, &destino);
        let mut azar = Lcg(7);
        RgbaImage::from_fn(w, h, |x, y| {
            let ruido = 40 + (azar.sig() % 30) as u8;
            let fondo = Rgba([ruido, ruido.saturating_sub(5), ruido + 10, 255]);
            let Some(hc) = &hacia_carta else {
                return fondo;
            };
            let (u, v) = hc.aplicar(x as f32 + 0.5, y as f32 + 0.5);
            if u < 0.0 || v < 0.0 || u >= carta.width() as f32 || v >= carta.height() as f32 {
                fondo
            } else {
                *carta.get_pixel(u as u32, v as u32)
            }
        })
    }

    /// Las cuatro esquinas de un rectángulo de carta de `alto` px centrado en
    /// (cx, cy) y girado `grados` (en el sentido del reloj).
    pub fn girada(cx: f32, cy: f32, alto: f32, grados: f32) -> [Punto; 4] {
        let ancho = alto * super::PROPORCION;
        let (s, c) = grados.to_radians().sin_cos();
        [
            (-ancho / 2.0, -alto / 2.0),
            (ancho / 2.0, -alto / 2.0),
            (ancho / 2.0, alto / 2.0),
            (-ancho / 2.0, alto / 2.0),
        ]
        .map(|(x, y)| Punto {
            x: cx + x * c - y * s,
            y: cy + x * s + y * c,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducir_respeta_la_proporcion_y_la_media() {
        let img = RgbaImage::from_pixel(1000, 500, image::Rgba([100, 100, 100, 255]));
        let (g, sx, sy) = gris_reducido(&img, 200);
        assert_eq!((g.ancho, g.alto), (200, 100));
        assert!((sx - 5.0).abs() < 1e-3 && (sy - 5.0).abs() < 1e-3);
        assert!(g.px.iter().all(|v| (v - 100.0).abs() < 0.5));
    }

    #[test]
    fn una_imagen_movida_es_menos_nitida() {
        let carta = pruebas::carta(1);
        let plano: Vec<f32> = carta.pixels().map(luma).collect();
        let borroso = crate::imaging::blur::gauss(&plano, ANCHO as usize, ALTO as usize, 4.0);
        let movida = RgbaImage::from_fn(ANCHO, ALTO, |x, y| {
            let v = borroso[(y * ANCHO + x) as usize] as u8;
            image::Rgba([v, v, v, 255])
        });
        assert!(nitidez(&carta) > 2.0 * nitidez(&movida));
    }
}
