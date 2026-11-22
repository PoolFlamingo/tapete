//! **Encontrar la carta en la foto**: sus cuatro esquinas, aunque esté girada,
//! en perspectiva o con fondo.
//!
//! Como un escáner de documentos: la foto en grises a 480 px, desenfocada; los
//! bordes (Sobel) por encima de un percentil; sus contornos; de cada contorno
//! grande, el polígono convexo simplificado hasta cuatro vértices (o, si no
//! sale, el rectángulo mínimo que lo encierra si lo llena bien); y de esos
//! cuadriláteros, el mayor con la proporción de una carta (63 × 88 mm, con
//! margen para la perspectiva).

use image::RgbaImage;
use rten_imageproc::{
    convex_hull, find_contours, min_area_rect, simplify_polygon, PointF, RetrievalMode,
};
use rten_tensor::prelude::*;
use rten_tensor::NdTensor;

use super::{gris_reducido, Esquinas, Gris, Punto, PROPORCION};

/// Lado mayor de la imagen en la que se busca (en píxeles).
pub const LADO_TRABAJO: u32 = 480;

/// Lo poco que puede ocupar la carta en la foto (fracción del área).
const AREA_MINIMA: f32 = 0.08;

/// Cuánto puede alejarse su proporción de la de una carta (relativo): la
/// perspectiva acorta un lado. Una carta vista desde abajo llega a un 18 %;
/// una pantalla de 16:9 queda a un 21 %.
const TOLERANCIA_PROPORCION: f32 = 0.20;

/// Qué parte de los píxeles se toma como borde (los más marcados).
const PERCENTIL_BORDE: f32 = 0.86;

/// Las esquinas de la carta en la foto, en píxeles de la original, o `None`
/// si no se ve ninguna.
pub fn detectar(img: &RgbaImage) -> Option<Esquinas> {
    let (g, sx, sy) = gris_reducido(img, LADO_TRABAJO);
    let q = en_gris(&g)?;
    Some(q.map(|p| Punto {
        x: p.x * sx,
        y: p.y * sy,
    }))
}

/// La detección sobre la imagen reducida (en sus píxeles).
pub(crate) fn en_gris(g: &Gris) -> Option<Esquinas> {
    let (w, h) = (g.ancho, g.alto);
    if w < 16 || h < 16 {
        return None;
    }
    let suave = crate::imaging::blur::gauss(&g.px, w, h, 1.2);
    let bordes = sobel(&suave, w, h);
    let mascara = umbral(&bordes, w, h);
    let contornos = find_contours(mascara.view(), RetrievalMode::List);

    let area_foto = (w * h) as f32;
    let mut mejor: Option<(f32, Esquinas)> = None;
    for contorno in contornos.iter() {
        if contorno.len() < 16 {
            continue;
        }
        let puntos: Vec<PointF> = contorno.iter().map(|p| p.to_f32()).collect();
        let casco = convex_hull(&puntos);
        let area = area_poligono(&casco);
        if area < AREA_MINIMA * area_foto || area > 0.995 * area_foto {
            continue;
        }
        let Some(q) = cuadrilatero(&casco, area) else {
            continue;
        };
        if !parece_carta(&q) {
            continue;
        }
        let area_q = area_poligono(&q);
        if mejor.as_ref().is_none_or(|(a, _)| area_q > *a) {
            mejor = Some((area_q, ordenar(&q)?));
        }
    }
    mejor.map(|(_, q)| q)
}

/// Magnitud del gradiente (Sobel 3 × 3); el borde de la imagen queda a 0.
fn sobel(p: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut m = vec![0.0f32; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let a = |dx: usize, dy: usize| p[(y + dy - 1) * w + (x + dx - 1)];
            let gx = (a(2, 0) + 2.0 * a(2, 1) + a(2, 2)) - (a(0, 0) + 2.0 * a(0, 1) + a(0, 2));
            let gy = (a(0, 2) + 2.0 * a(1, 2) + a(2, 2)) - (a(0, 0) + 2.0 * a(1, 0) + a(2, 0));
            m[y * w + x] = (gx * gx + gy * gy).sqrt();
        }
    }
    m
}

/// Los píxeles de borde (por encima del percentil, y no del ruido de una foto
/// lisa), engordados un píxel para cerrar los huecos del contorno.
fn umbral(m: &[f32], w: usize, h: usize) -> NdTensor<bool, 2> {
    let mut orden: Vec<f32> = m.to_vec();
    let k = ((orden.len() as f32 * PERCENTIL_BORDE) as usize).min(orden.len() - 1);
    let (_, corte, _) = orden.select_nth_unstable_by(k, f32::total_cmp);
    let corte = corte.max(24.0);
    let mut mascara = NdTensor::<bool, 2>::zeros([h, w]);
    for y in 0..h {
        for x in 0..w {
            if m[y * w + x] <= corte {
                continue;
            }
            for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    mascara[[yy, xx]] = true;
                }
            }
        }
    }
    mascara
}

/// Área de un polígono (fórmula del cordón), en valor absoluto.
fn area_poligono(p: &[PointF]) -> f32 {
    let n = p.len();
    if n < 3 {
        return 0.0;
    }
    let doble: f32 = (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    doble.abs() / 2.0
}

fn perimetro(p: &[PointF]) -> f32 {
    (0..p.len())
        .map(|i| p[i].distance(p[(i + 1) % p.len()]))
        .sum()
}

/// Cuatro vértices para el casco convexo: simplificándolo poco a poco o, si
/// no sale, el rectángulo mínimo que lo encierra (una esquina tapada por un
/// dedo, una funda con las esquinas muy redondas).
fn cuadrilatero(casco: &[PointF], area: f32) -> Option<Vec<PointF>> {
    if casco.len() < 4 {
        return None;
    }
    // Douglas-Peucker conserva siempre el primer punto: se empieza por el más
    // alejado del centro, que es una esquina (si no, recorta una).
    let (cx, cy) = (
        casco.iter().map(|p| p.x).sum::<f32>() / casco.len() as f32,
        casco.iter().map(|p| p.y).sum::<f32>() / casco.len() as f32,
    );
    let centro = PointF::from_yx(cy, cx);
    let lejos = (0..casco.len())
        .max_by(|&a, &b| {
            casco[a]
                .distance(centro)
                .total_cmp(&casco[b].distance(centro))
        })
        .unwrap_or(0);
    let mut girado = casco.to_vec();
    girado.rotate_left(lejos);
    let casco = girado.as_slice();
    let p = perimetro(casco);
    for paso in 1..=10 {
        let s = simplify_polygon(casco, p * 0.008 * paso as f32);
        match s.len() {
            4 => {
                let a = area_poligono(&s);
                // Que no se haya comido un trozo grande del contorno.
                return (a > 0.85 * area && a < 1.15 * area).then_some(s);
            }
            n if n < 4 => break,
            _ => {}
        }
    }
    let r = min_area_rect(casco)?;
    (area >= 0.85 * r.area()).then(|| r.corners().to_vec())
}

/// Lo que el contorno queda por fuera del borde de verdad, en píxeles de la
/// imagen reducida: el desenfoque y el engordado de un píxel lo ensanchan.
const BANDA: f32 = 2.0;

/// ¿Tiene la proporción de una carta (contando con la perspectiva)? Se mide
/// sin la banda del contorno: con ella, todo rectángulo parece más cuadrado
/// (una pantalla de 16:9 pasaba por carta).
fn parece_carta(q: &[PointF]) -> bool {
    let l = |i: usize| q[i].distance(q[(i + 1) % 4]);
    let (a, b) = (
        (l(0) + l(2)) / 2.0 - 2.0 * BANDA,
        (l(1) + l(3)) / 2.0 - 2.0 * BANDA,
    );
    if a.min(b) < 1.0 {
        return false;
    }
    let proporcion = a.min(b) / a.max(b);
    (proporcion - PROPORCION).abs() <= PROPORCION * TOLERANCIA_PROPORCION
}

/// Las pone en el sentido del reloj empezando por la esquina de arriba a la
/// izquierda de la carta: el primer lado es uno de los cortos, el que queda
/// más arriba en la foto (o el de la izquierda si la carta está tumbada). Una
/// carta boca abajo sale al revés; lo arregla quien lee (`girar180`).
pub(crate) fn ordenar(q: &[PointF]) -> Option<Esquinas> {
    if q.len() != 4 {
        return None;
    }
    let (cx, cy) = (
        q.iter().map(|p| p.x).sum::<f32>() / 4.0,
        q.iter().map(|p| p.y).sum::<f32>() / 4.0,
    );
    let mut p: Vec<Punto> = q.iter().map(|p| Punto { x: p.x, y: p.y }).collect();
    // Con la y hacia abajo, el ángulo creciente va en el sentido del reloj.
    p.sort_by(|a, b| {
        (a.y - cy)
            .atan2(a.x - cx)
            .total_cmp(&(b.y - cy).atan2(b.x - cx))
    });
    let l = |i: usize| p[i].distancia(p[(i + 1) % 4]);
    // Los lados cortos: 0 y 2, o 1 y 3.
    let cortos = if l(0) + l(2) <= l(1) + l(3) {
        [0, 2]
    } else {
        [1, 3]
    };
    let medio = |i: usize| {
        let (a, b) = (p[i], p[(i + 1) % 4]);
        Punto {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        }
    };
    let (m0, m1) = (medio(cortos[0]), medio(cortos[1]));
    let corto = l(cortos[0]).min(l(cortos[1]));
    let inicio = if (m0.y - m1.y).abs() > 0.25 * corto {
        if m0.y < m1.y {
            cortos[0]
        } else {
            cortos[1]
        }
    } else if m0.x <= m1.x {
        cortos[0]
    } else {
        cortos[1]
    };
    Some([0, 1, 2, 3].map(|k| p[(inicio + k) % 4]))
}

#[cfg(test)]
mod tests {
    use super::super::{pruebas, Punto};
    use super::*;

    /// Cada esquina esperada tiene una detectada a menos de `tolerancia` px.
    fn mismas(detectadas: &Esquinas, esperadas: &Esquinas, tolerancia: f32) -> bool {
        esperadas
            .iter()
            .all(|e| detectadas.iter().any(|d| d.distancia(*e) <= tolerancia))
    }

    fn probar(grados: f32) {
        let carta = pruebas::carta(1);
        let esquinas = pruebas::girada(500.0, 420.0, 600.0, grados);
        let foto = pruebas::foto(&carta, 1000, 840, esquinas);
        let d = detectar(&foto).unwrap_or_else(|| panic!("sin carta a {grados}°"));
        assert!(
            mismas(&d, &esquinas, 600.0 * 0.02),
            "{grados}°: {d:?} frente a {esquinas:?}"
        );
        // El primer lado es corto y va en el sentido del reloj.
        assert!(
            d[0].distancia(d[1]) < d[1].distancia(d[2]),
            "{grados}°: {d:?}"
        );
        let cruz = (d[1].x - d[0].x) * (d[2].y - d[1].y) - (d[1].y - d[0].y) * (d[2].x - d[1].x);
        assert!(cruz > 0.0, "{grados}°: no va en el sentido del reloj");
    }

    #[test]
    fn de_frente() {
        probar(0.0);
        let d = detectar(&pruebas::foto(
            &pruebas::carta(1),
            1000,
            840,
            pruebas::girada(500.0, 420.0, 600.0, 0.0),
        ))
        .expect("carta");
        // De frente, la primera es la de arriba a la izquierda.
        assert!(d[0].x < 400.0 && d[0].y < 300.0, "{d:?}");
    }

    #[test]
    fn girada() {
        probar(15.0);
        probar(-25.0);
    }

    #[test]
    fn tumbada() {
        probar(90.0);
    }

    #[test]
    fn boca_abajo() {
        probar(170.0);
    }

    #[test]
    fn en_perspectiva() {
        let carta = pruebas::carta(4);
        let esquinas = [
            (330.0, 160.0),
            (700.0, 190.0),
            (760.0, 700.0),
            (250.0, 690.0),
        ]
        .map(|(x, y)| Punto { x, y });
        let foto = pruebas::foto(&carta, 1000, 840, esquinas);
        let d = detectar(&foto).expect("carta");
        assert!(mismas(&d, &esquinas, 14.0), "{d:?}");
    }

    #[test]
    fn una_foto_grande_da_las_esquinas_en_sus_pixeles() {
        let carta = pruebas::carta(2);
        let esquinas = pruebas::girada(1500.0, 1200.0, 1800.0, 8.0);
        let foto = pruebas::foto(&carta, 3000, 2400, esquinas);
        let d = detectar(&foto).expect("carta");
        assert!(mismas(&d, &esquinas, 1800.0 * 0.02), "{d:?}");
    }

    #[test]
    fn sin_carta_no_hay_esquinas() {
        let mut azar = pruebas::Lcg(3);
        let foto = RgbaImage::from_fn(800, 600, |_, _| {
            let v = 40 + (azar.sig() % 30) as u8;
            image::Rgba([v, v, v, 255])
        });
        assert_eq!(detectar(&foto), None);
    }

    #[test]
    fn una_pantalla_de_16_9_no_es_una_carta() {
        // Una tele o un móvil de frente: 16:9 = 0,5625, fuera del margen.
        let foto = RgbaImage::from_fn(1000, 800, |x, y| {
            if (300..525).contains(&x) && (200..600).contains(&y) {
                image::Rgba([220, 220, 220, 255])
            } else {
                image::Rgba([40, 40, 40, 255])
            }
        });
        assert_eq!(detectar(&foto), None);
    }

    #[test]
    fn un_rectangulo_que_no_es_una_carta_no_vale() {
        // Una caja apaisada de 2:1 no es una carta.
        let foto = RgbaImage::from_fn(1000, 800, |x, y| {
            if (200..800).contains(&x) && (250..550).contains(&y) {
                image::Rgba([220, 220, 220, 255])
            } else {
                image::Rgba([40, 40, 40, 255])
            }
        });
        assert_eq!(detectar(&foto), None);
    }
}
