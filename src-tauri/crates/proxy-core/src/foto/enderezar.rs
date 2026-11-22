//! **Dejar la carta plana**: de las cuatro esquinas que da `detectar` a una
//! carta de `ANCHO × ALTO` vista de frente (homografía y remuestreo bilineal,
//! con varias muestras por píxel si la carta de la foto es mucho mayor).

use image::RgbaImage;
use rayon::prelude::*;

use super::{Esquinas, Punto};

/// Medida de la carta enderezada: la de una imagen de Scryfall «normal»
/// ampliada, con el título y el pie con letra de sobra para el OCR.
pub const ANCHO: u32 = 630;
pub const ALTO: u32 = 880;

/// Una transformación proyectiva del plano (la matriz 3 × 3 con h₈ = 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Homografia([f64; 8]);

impl Homografia {
    /// La que lleva cada punto de `desde` al mismo de `hasta`. `None` si los
    /// puntos están alineados (no hay ninguna).
    pub fn de(desde: &[Punto; 4], hasta: &[Punto; 4]) -> Option<Homografia> {
        // x' = (h0 x + h1 y + h2) / (h6 x + h7 y + 1); y' igual con h3, h4, h5.
        let mut m = [[0.0f64; 9]; 8];
        for (i, (p, q)) in desde.iter().zip(hasta).enumerate() {
            let (x, y, u, v) = (p.x as f64, p.y as f64, q.x as f64, q.y as f64);
            m[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
            m[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
        }
        resolver(m).map(Homografia)
    }

    pub fn aplicar(&self, x: f32, y: f32) -> (f32, f32) {
        let h = &self.0;
        let (x, y) = (x as f64, y as f64);
        let w = h[6] * x + h[7] * y + 1.0;
        (
            ((h[0] * x + h[1] * y + h[2]) / w) as f32,
            ((h[3] * x + h[4] * y + h[5]) / w) as f32,
        )
    }
}

/// Eliminación gaussiana con pivote parcial de un sistema 8 × 8 (la última
/// columna es el término independiente).
fn resolver(mut m: [[f64; 9]; 8]) -> Option<[f64; 8]> {
    for col in 0..8 {
        let pivote = (col..8).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivote][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivote);
        let fila = m[col];
        for (f, otra) in m.iter_mut().enumerate() {
            if f == col {
                continue;
            }
            let k = otra[col] / fila[col];
            if k != 0.0 {
                for (a, b) in otra.iter_mut().zip(fila.iter()).skip(col) {
                    *a -= k * b;
                }
            }
        }
    }
    let mut x = [0.0f64; 8];
    for (i, xi) in x.iter_mut().enumerate() {
        *xi = m[i][8] / m[i][i];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Un píxel de `img` en (x, y) con interpolación bilineal (fuera, el borde).
fn muestra(img: &RgbaImage, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let (x, y) = (x - 0.5, y - 0.5);
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let px = |xi: i64, yi: i64| img.get_pixel(xi.clamp(0, w - 1) as u32, yi.clamp(0, h - 1) as u32);
    let (a, b, c, d) = (
        px(x0, y0),
        px(x0 + 1, y0),
        px(x0, y0 + 1),
        px(x0 + 1, y0 + 1),
    );
    let mut r = [0.0f32; 4];
    for (i, v) in r.iter_mut().enumerate() {
        let arriba = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let abajo = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        *v = arriba * (1.0 - fy) + abajo * fy;
    }
    r
}

/// La carta de la foto, plana y de `ANCHO × ALTO`, con `esquinas[0]` arriba a
/// la izquierda. `None` si las esquinas no forman un cuadrilátero.
pub fn enderezar(foto: &RgbaImage, esquinas: &Esquinas) -> Option<RgbaImage> {
    enderezar_a(foto, esquinas, ANCHO, ALTO)
}

/// Como `enderezar`, a la medida que se pida.
pub fn enderezar_a(
    foto: &RgbaImage,
    esquinas: &Esquinas,
    ancho: u32,
    alto: u32,
) -> Option<RgbaImage> {
    let destino = [
        Punto { x: 0.0, y: 0.0 },
        Punto {
            x: ancho as f32,
            y: 0.0,
        },
        Punto {
            x: ancho as f32,
            y: alto as f32,
        },
        Punto {
            x: 0.0,
            y: alto as f32,
        },
    ];
    let h = Homografia::de(&destino, esquinas)?;
    // Cuántos píxeles de la foto caen en uno de la carta: si son muchos, se
    // toman n × n muestras por píxel para no perder la letra pequeña.
    let lado = esquinas[0]
        .distancia(esquinas[3])
        .max(esquinas[1].distancia(esquinas[2]));
    let n = ((lado / alto as f32).ceil() as usize).clamp(1, 3);
    let mut px = vec![0u8; (ancho * alto * 4) as usize];
    px.par_chunks_mut((ancho * 4) as usize)
        .enumerate()
        .for_each(|(v, fila)| {
            for u in 0..ancho as usize {
                let mut s = [0.0f32; 4];
                for j in 0..n {
                    for i in 0..n {
                        let du = u as f32 + (i as f32 + 0.5) / n as f32;
                        let dv = v as f32 + (j as f32 + 0.5) / n as f32;
                        let (x, y) = h.aplicar(du, dv);
                        for (a, b) in s.iter_mut().zip(muestra(foto, x, y)) {
                            *a += b;
                        }
                    }
                }
                let total = (n * n) as f32;
                for (destino, v) in fila[u * 4..u * 4 + 4].iter_mut().zip(s) {
                    *destino = (v / total).round().clamp(0.0, 255.0) as u8;
                }
            }
        });
    RgbaImage::from_raw(ancho, alto, px)
}

/// La carta al revés (estaba boca abajo en la foto).
pub fn girar180(img: &RgbaImage) -> RgbaImage {
    image::imageops::rotate180(img)
}

#[cfg(test)]
mod tests {
    use super::super::pruebas;
    use super::*;

    fn error_medio(a: &RgbaImage, b: &RgbaImage, margen: u32) -> f32 {
        let mut suma = 0.0f32;
        let mut n = 0.0f32;
        for y in margen..a.height() - margen {
            for x in margen..a.width() - margen {
                let (p, q) = (a.get_pixel(x, y), b.get_pixel(x, y));
                for k in 0..3 {
                    suma += (p[k] as f32 - q[k] as f32).abs();
                }
                n += 3.0;
            }
        }
        suma / n
    }

    #[test]
    fn la_homografia_lleva_cada_esquina_a_la_suya() {
        let a = [(10.0, 20.0), (300.0, 40.0), (280.0, 400.0), (5.0, 380.0)]
            .map(|(x, y)| Punto { x, y });
        let b =
            [(0.0, 0.0), (630.0, 0.0), (630.0, 880.0), (0.0, 880.0)].map(|(x, y)| Punto { x, y });
        let h = Homografia::de(&a, &b).expect("homografía");
        for (p, q) in a.iter().zip(&b) {
            let (x, y) = h.aplicar(p.x, p.y);
            assert!(
                (x - q.x).abs() < 1e-2 && (y - q.y).abs() < 1e-2,
                "{p:?} → ({x}, {y})"
            );
        }
    }

    #[test]
    fn puntos_alineados_no_dan_homografia() {
        let a = [(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)].map(|(x, y)| Punto { x, y });
        let b = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].map(|(x, y)| Punto { x, y });
        assert!(Homografia::de(&a, &b).is_none());
    }

    #[test]
    fn enderezar_deshace_la_perspectiva() {
        let carta = pruebas::carta(3);
        // Un trapecio: la carta vista desde abajo y algo girada.
        let esquinas = [
            (310.0, 140.0),
            (760.0, 175.0),
            (820.0, 820.0),
            (230.0, 790.0),
        ]
        .map(|(x, y)| Punto { x, y });
        let foto = pruebas::foto(&carta, 1100, 950, esquinas);
        let plana = enderezar(&foto, &esquinas).expect("enderezada");
        assert_eq!((plana.width(), plana.height()), (ANCHO, ALTO));
        let e = error_medio(&carta, &plana, 12);
        assert!(e <= 6.0, "error medio {e}");
    }

    #[test]
    fn las_esquinas_de_la_imagen_la_dejan_igual() {
        let carta = pruebas::carta(5);
        let esquinas =
            [(0.0, 0.0), (630.0, 0.0), (630.0, 880.0), (0.0, 880.0)].map(|(x, y)| Punto { x, y });
        let plana = enderezar(&carta, &esquinas).expect("enderezada");
        assert!(error_medio(&carta, &plana, 1) < 1.0);
    }

    #[test]
    fn girar180_vuelve_a_la_original_dos_veces() {
        let carta = pruebas::carta(2);
        assert_eq!(girar180(&girar180(&carta)), carta);
    }
}
