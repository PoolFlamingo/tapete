//! **Identificar una carta real por una foto** (la de la galería, la que se
//! suelta en el escritorio o, en Android, el escáner en vivo).
//!
//! 1. `proxy_core::foto::detectar` busca la carta en la foto y
//!    `enderezar` la deja plana y derecha (sin carta a la vista, la foto
//!    entera).
//! 2. El OCR de siempre (`commands::identify::leer_imagen`) lee título y pie,
//!    y `proponer` busca la carta como con una propia. Si no sale nada, la
//!    carta estaba boca abajo: se gira y se lee otra vez.
//! 3. La impresión (`impresion`): la del pie (colección y número) si se lee;
//!    si no, la de ilustración más parecida entre las de ese nombre.

pub mod impresion;
pub mod vivo;

use base64::Engine;
use image::RgbaImage;
use proxy_core::foto::{self, Punto};
use serde::Serialize;
use ts_rs::TS;

use crate::commands::cards::CardSummary;
use crate::commands::identify::{leer_imagen, proponer, CardIdentify, Confidence};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::state::AppState;

/// Ancho de la miniatura de la carta enderezada que ve la pantalla.
const ANCHO_MINIATURA: u32 = 300;

/// Cómo se supo la impresión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PorDonde {
    /// El pie: colección y número leídos.
    Pie,
    /// La ilustración más parecida entre las impresiones del nombre.
    Imagen,
    /// Solo el nombre (sin red para comparar, o sin imágenes).
    Nombre,
    /// No se reconoció ninguna carta.
    Nada,
}

/// Una impresión candidata y lo que se aleja su ilustración de la foto
/// (0 = igual; 1 = nada que ver; `null` si no se comparó).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImpresionPuntuada {
    pub carta: CardSummary,
    pub distancia: Option<f32>,
}

/// Lo que se ha sacado de una foto.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Reconocida {
    /// Dónde estaba la carta en la foto, en fracciones de su ancho y alto
    /// (`null`: no se vio y se usó la foto entera).
    pub esquinas: Option<[Punto; 4]>,
    /// La carta enderezada, en pequeño (`data:image/jpeg;base64,…`).
    pub miniatura: String,
    /// Lo leído: los nombres del pie y luego el título.
    pub lectura: Vec<String>,
    /// Cartas (nombres) propuestas, la más probable primero.
    pub candidatas: Vec<CardSummary>,
    /// La impresión propuesta (id de Scryfall).
    pub mejor: Option<String>,
    pub confianza: Confidence,
    /// Las impresiones del nombre propuesto, la más probable primero.
    pub impresiones: Vec<ImpresionPuntuada>,
    pub por: PorDonde,
}

/// La foto entera estirada a carta (no se vio ninguna; puede ser ya un escaneo).
fn foto_entera(img: &RgbaImage) -> RgbaImage {
    image::imageops::resize(
        img,
        foto::ANCHO,
        foto::ALTO,
        image::imageops::FilterType::Triangle,
    )
}

/// Encuentra y endereza la carta. Pesado: va en un hilo aparte.
pub(crate) fn preparar(img: &RgbaImage) -> (Option<[Punto; 4]>, RgbaImage) {
    let esquinas = foto::detectar(img);
    let carta = esquinas
        .as_ref()
        .and_then(|e| foto::enderezar(img, e))
        .unwrap_or_else(|| foto_entera(img));
    let (w, h) = (img.width().max(1) as f32, img.height().max(1) as f32);
    let fracciones = esquinas.map(|e| {
        e.map(|p| Punto {
            x: p.x / w,
            y: p.y / h,
        })
    });
    (fracciones, carta)
}

fn miniatura(carta: &RgbaImage) -> String {
    let alto = ANCHO_MINIATURA * carta.height() / carta.width().max(1);
    let p = image::imageops::resize(
        carta,
        ANCHO_MINIATURA,
        alto.max(1),
        image::imageops::FilterType::Triangle,
    );
    match proxy_core::imaging::encode::jpeg(&p, 80) {
        Ok(b) => format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(b)
        ),
        Err(_) => String::new(),
    }
}

fn interno(e: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::Internal).with("detalle", e.to_string())
}

/// ¿Ha dado algo esta lectura?
fn algo(r: &CardIdentify) -> bool {
    r.confidence != Confidence::None || r.pie.is_some()
}

/// Lee la carta y propone; si no sale nada, la lee boca abajo.
async fn leer_y_proponer(
    state: &AppState,
    carta: RgbaImage,
) -> AppResult<(RgbaImage, CardIdentify)> {
    let lectura = leer_imagen(state, carta.clone()).await?;
    let r = proponer(state, lectura, &[], false).await?;
    if algo(&r) {
        return Ok((carta, r));
    }
    let girada = foto::girar180(&carta);
    let lectura = leer_imagen(state, girada.clone()).await?;
    let r2 = proponer(state, lectura, &[], false).await?;
    Ok(if algo(&r2) || r2.candidates.len() > r.candidates.len() {
        (girada, r2)
    } else {
        (carta, r)
    })
}

/// Identifica la carta de una foto.
pub async fn reconocer(state: &AppState, img: RgbaImage) -> AppResult<Reconocida> {
    let inicio = std::time::Instant::now();
    let (esquinas, carta) = tauri::async_runtime::spawn_blocking(move || preparar(&img))
        .await
        .map_err(interno)?;
    tracing::info!(
        carta = esquinas.is_some(),
        detectar_ms = inicio.elapsed().as_millis() as u64,
        "reconocer: foto"
    );
    reconocer_enderezada(state, esquinas, carta).await
}

/// Identifica una carta ya encontrada y enderezada (`esquinas`, en fracciones
/// de la foto, solo para enseñarlas).
pub async fn reconocer_enderezada(
    state: &AppState,
    esquinas: Option<[Punto; 4]>,
    carta: RgbaImage,
) -> AppResult<Reconocida> {
    let inicio = std::time::Instant::now();
    let (carta, ident) = leer_y_proponer(state, carta).await?;
    let leida = inicio.elapsed();
    let (mejor, impresiones, por) = impresion::elegir(state, &carta, &ident).await;
    tracing::info!(
        leer_ms = leida.as_millis() as u64,
        impresion_ms = (inicio.elapsed() - leida).as_millis() as u64,
        por = ?por,
        "reconocer"
    );
    let confianza = match por {
        // El pie da la carta exacta (y coincide con el nombre leído).
        PorDonde::Pie => Confidence::Exact,
        _ => ident.confidence,
    };
    Ok(Reconocida {
        esquinas,
        miniatura: miniatura(&carta),
        lectura: ident.lines,
        candidatas: ident.candidates,
        mejor,
        confianza,
        impresiones,
        por,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_carta_a_la_vista_se_usa_la_foto_entera() {
        let img = RgbaImage::from_pixel(400, 300, image::Rgba([90, 90, 90, 255]));
        let (esquinas, carta) = preparar(&img);
        assert!(esquinas.is_none());
        assert_eq!((carta.width(), carta.height()), (foto::ANCHO, foto::ALTO));
    }

    #[test]
    fn las_esquinas_van_en_fracciones_de_la_foto() {
        // Una carta clara de 63 × 88 sobre fondo oscuro.
        let img = RgbaImage::from_fn(1000, 1000, |x, y| {
            if (300..615).contains(&x) && (280..720).contains(&y) {
                image::Rgba([230, 225, 210, 255])
            } else {
                image::Rgba([30, 30, 35, 255])
            }
        });
        let (esquinas, _) = preparar(&img);
        let e = esquinas.expect("carta");
        assert!(e
            .iter()
            .all(|p| (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y)));
        assert!(
            (e[0].x - 0.30).abs() < 0.02 && (e[0].y - 0.28).abs() < 0.02,
            "{e:?}"
        );
    }

    /// Paso 0 del plan: lo que tarda el OCR en leer una carta enderezada, para
    /// decidir si llega para el escáner en vivo. A mano y en el móvil:
    /// `PB_DATOS=<carpeta con modelos/> PB_FOTO=<foto> cargo test --lib
    /// mide_el_ocr -- --ignored --nocapture` (en Android, el binario de test
    /// compilado con `--target aarch64-linux-android --no-run` y `adb push`).
    #[test]
    #[ignore]
    fn mide_el_ocr() {
        let datos = std::path::PathBuf::from(std::env::var("PB_DATOS").expect("PB_DATOS"));
        let foto = std::fs::read(std::env::var("PB_FOTO").expect("PB_FOTO")).expect("foto");
        let img = proxy_core::imaging::decode::decode(&foto).expect("decode");
        let t = std::time::Instant::now();
        let (esquinas, carta) = preparar(&img);
        println!(
            "detectar y enderezar: {:?} (carta: {})",
            t.elapsed(),
            esquinas.is_some()
        );
        let alto = carta.height();
        let franja = |y0: f32, y1: f32| {
            let (a, b) = ((y0 * alto as f32) as u32, (y1 * alto as f32) as u32);
            image::imageops::crop_imm(&carta, 0, a, carta.width(), b - a).to_image()
        };
        let mut veces: Vec<(&str, Vec<std::time::Duration>)> = vec![
            ("carta entera", vec![]),
            ("título", vec![]),
            ("pie", vec![]),
        ];
        for _ in 0..20 {
            for (nombre, v) in &mut veces {
                let img = match *nombre {
                    "título" => franja(0.0, crate::ocr::FRANJA_TITULO),
                    "pie" => franja(0.88, 1.0),
                    _ => carta.clone(),
                };
                let t = std::time::Instant::now();
                let l = crate::ocr::leer_carta(&datos, &img).expect("ocr");
                v.push(t.elapsed());
                if v.len() == 1 {
                    println!(
                        "{nombre}: {:?}",
                        l.lineas.iter().map(|x| &x.texto).collect::<Vec<_>>()
                    );
                }
            }
        }
        for (nombre, mut v) in veces {
            v.sort();
            println!("{nombre}: mediana {:?}", v[v.len() / 2]);
        }
    }

    /// La cadena entera contra Scryfall de verdad (red), en una carpeta
    /// temporal: `PB_MODELOS=<carpeta modelos/ del OCR> PB_FOTO=<foto> cargo
    /// test --lib reconoce_una_foto_de_verdad -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reconoce_una_foto_de_verdad() {
        let modelos = std::path::PathBuf::from(std::env::var("PB_MODELOS").expect("PB_MODELOS"));
        let foto = std::fs::read(std::env::var("PB_FOTO").expect("PB_FOTO")).expect("foto");
        let dir = tempfile::tempdir().expect("tmp");
        let data = dir.path().join("data");
        std::fs::create_dir_all(data.join("modelos")).expect("modelos");
        for f in std::fs::read_dir(&modelos).expect("modelos") {
            let f = f.expect("fichero").path();
            if let Some(n) = f.file_name() {
                std::fs::copy(&f, data.join("modelos").join(n)).expect("copiar");
            }
        }
        let state = AppState::open(crate::state::Paths {
            data,
            cache: dir.path().join("cache"),
        })
        .expect("estado");
        let img = proxy_core::imaging::decode::decode(&foto).expect("decode");
        let t = std::time::Instant::now();
        let r = tauri::async_runtime::block_on(reconocer(&state, img)).expect("reconocer");
        println!("{:?} en {:?}", r.por, t.elapsed());
        println!("leído: {:?}", r.lectura);
        println!("confianza: {:?}, mejor: {:?}", r.confianza, r.mejor);
        for i in r.impresiones.iter().take(60) {
            println!(
                "  {} · {} {} {} → {:?}",
                i.carta.name, i.carta.set, i.carta.collector_number, i.carta.lang, i.distancia
            );
        }
        assert!(r.mejor.is_some());
    }

    #[test]
    fn la_miniatura_es_un_jpeg_en_data_url() {
        let m = miniatura(&RgbaImage::from_pixel(
            630,
            880,
            image::Rgba([1, 2, 3, 255]),
        ));
        assert!(m.starts_with("data:image/jpeg;base64,"));
    }
}
