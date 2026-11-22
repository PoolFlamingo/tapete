//! **Qué impresión es la de la foto.**
//!
//! Primero el pie: las cartas desde 2014 llevan la colección y el número (y
//! desde 2023 el idioma) y eso da la impresión exacta. Si el pie no se lee,
//! se comparan las ilustraciones: se bajan las miniaturas `small` de las
//! impresiones del nombre (una por ilustración, hasta `MAX_ILUSTRACIONES`) a
//! `app_cache_dir/reconocer/` (un derivado: borrarlo no pierde nada) y se
//! ordenan por lo que se parecen a la foto (`proxy_core::foto::Firma`).

use std::path::Path;

use futures::StreamExt;
use image::RgbaImage;
use proxy_core::foto::firma::ENTERA;
use proxy_core::foto::Firma;

use super::{ImpresionPuntuada, PorDonde};
use crate::commands::cards::{consulta_impresiones, resumir, CardSummary};
use crate::commands::identify::{normalizar, CardIdentify};
use crate::scryfall::client::CardApi;
use crate::scryfall::models::ScryCard;
use crate::state::AppState;
use crate::store::blobs::escribir_atomico;

/// Cuántas ilustraciones distintas se comparan como mucho (las básicas y
/// Sol Ring tienen cientos de impresiones).
pub const MAX_ILUSTRACIONES: usize = 40;

/// Por encima de esta distancia ninguna ilustración es la de la foto (una
/// proxy, un arte que Scryfall no tiene). Con cartas reales la buena queda
/// hacia 0,11 y las demás hacia 0,5.
pub const UMBRAL_IMAGEN: f32 = 0.30;

/// Cuántas impresiones de la ilustración ganadora se comparan enteras, para
/// elegir el marco (una por colección y número).
pub const MAX_MARCOS: usize = 12;

/// Cuántas miniaturas se bajan a la vez.
const A_LA_VEZ: usize = 4;

/// Cuántas impresiones se piden a Scryfall.
const MAX_IMPRESIONES: usize = 175;

/// La carpeta de las miniaturas para comparar.
pub fn carpeta(cache: &Path) -> std::path::PathBuf {
    cache.join("reconocer")
}

/// La impresión propuesta, las impresiones ordenadas y cómo se supo.
pub async fn elegir(
    state: &AppState,
    carta: &RgbaImage,
    ident: &CardIdentify,
) -> (Option<String>, Vec<ImpresionPuntuada>, PorDonde) {
    let Some(elegida) = ident
        .best
        .as_ref()
        .and_then(|b| ident.candidates.iter().find(|c| &c.id == b))
    else {
        return (None, Vec::new(), PorDonde::Nada);
    };
    // Una carta de otra fuente (una ficha de Cockatrice): no tiene impresiones.
    if elegida.fuente.is_some() {
        let una = ImpresionPuntuada {
            carta: elegida.clone(),
            distancia: None,
        };
        return (Some(elegida.id.clone()), vec![una], PorDonde::Nombre);
    }

    let api = &*state.scryfall;
    // El pie: la impresión exacta, si es de la carta leída (un pie mal leído
    // puede dar otra).
    let mut del_pie: Option<ScryCard> = None;
    if let Some(p) = &ident.pie {
        let idioma = p.idioma.as_deref().unwrap_or("en");
        if let Ok(Some(c)) = api.card_lang(&p.set, &p.numero, idioma).await {
            if mismo_nombre(&c, elegida) {
                del_pie = Some(c);
            }
        }
    }

    let idioma = del_pie
        .as_ref()
        .map(|c| c.lang.clone())
        .unwrap_or_else(|| elegida.lang.clone());
    let impresiones = impresiones_de(api, &elegida.name, &idioma).await;

    if let Some(exacta) = del_pie {
        let mut lista = vec![exacta.clone()];
        lista.extend(impresiones.into_iter().filter(|c| c.id != exacta.id));
        let lista = puntuadas(state, lista.into_iter().map(|c| (c, None)).collect());
        return (Some(exacta.id), lista, PorDonde::Pie);
    }

    let huellas = Huellas::de(carta);
    let dir = carpeta(&state.paths.cache);
    let ordenadas = ordenar_por_imagen(api, &dir, &huellas, impresiones, &idioma).await;
    // Solo cuenta si la ilustración se parece de verdad: si no, la foto es de
    // un arte que Scryfall no tiene (una proxy) y se elige a mano.
    let (mejor, por) = match ordenadas.first() {
        Some((c, Some(d))) if *d <= UMBRAL_IMAGEN => (Some(c.id.clone()), PorDonde::Imagen),
        _ => (Some(elegida.id.clone()), PorDonde::Nombre),
    };
    (mejor, puntuadas(state, ordenadas), por)
}

fn mismo_nombre(c: &ScryCard, elegida: &CardSummary) -> bool {
    normalizar(&c.name) == normalizar(&elegida.name)
}

/// Las impresiones de un nombre en ese idioma (o en inglés si en ese no hay).
async fn impresiones_de(api: &impl CardApi, nombre: &str, idioma: &str) -> Vec<ScryCard> {
    let lang = (idioma != "en" && !idioma.is_empty()).then(|| idioma.to_string());
    let q = consulta_impresiones(nombre, &lang, false);
    let v = api
        .search(&q, true, MAX_IMPRESIONES)
        .await
        .unwrap_or_default();
    if !v.is_empty() || lang.is_none() {
        return v;
    }
    let q = consulta_impresiones(nombre, &None, false);
    api.search(&q, true, MAX_IMPRESIONES)
        .await
        .unwrap_or_default()
}

fn puntuadas(state: &AppState, v: Vec<(ScryCard, Option<f32>)>) -> Vec<ImpresionPuntuada> {
    let (cartas, distancias): (Vec<ScryCard>, Vec<Option<f32>>) = v.into_iter().unzip();
    resumir(state, cartas)
        .into_iter()
        .zip(distancias)
        .map(|(carta, distancia)| ImpresionPuntuada { carta, distancia })
        .collect()
}

/// La URL de la miniatura con la que se compara (la cara de delante).
fn url_pequena(c: &ScryCard) -> Option<String> {
    let u = c.face_uris(0)?;
    u.small.clone().or_else(|| u.normal.clone())
}

/// El fichero de la miniatura: el id de Scryfall, sin nada que no sea letra,
/// cifra o guion (va en una ruta).
fn nombre_fichero(id: &str) -> Option<String> {
    let limpio: String = id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect();
    (!limpio.is_empty()).then(|| format!("{limpio}.jpg"))
}

/// Las dos huellas de una carta: la de su ilustración (para saber qué arte
/// es) y la de la carta entera (para el marco, entre las del mismo arte).
pub struct Huellas {
    pub arte: Firma,
    pub entera: Firma,
}

impl Huellas {
    pub fn de(carta: &RgbaImage) -> Huellas {
        Huellas {
            arte: Firma::de(carta),
            entera: Firma::de_zona(carta, ENTERA),
        }
    }
}

/// Las huellas de la miniatura de una impresión, de la caché o bajándola.
async fn huellas_de(api: &impl CardApi, dir: &Path, c: &ScryCard) -> Option<Huellas> {
    let url = url_pequena(c)?;
    let ruta = dir.join(nombre_fichero(&c.id)?);
    let bytes = match std::fs::read(&ruta) {
        Ok(b) => b,
        Err(_) => {
            let b = api.download(&url).await.ok()?;
            if let Err(e) = escribir_atomico(&ruta, &b) {
                tracing::warn!(codigo = ?e.code, "reconocer: no se guardó la miniatura");
            }
            b
        }
    };
    let img = proxy_core::imaging::decode::decode(&bytes).ok()?;
    Some(Huellas::de(&img))
}

/// La ilustración de una impresión (la suya, o la propia impresión si
/// Scryfall no la da).
fn ilustracion(c: &ScryCard) -> String {
    c.illustration_id.clone().unwrap_or_else(|| c.id.clone())
}

fn por_distancia(a: Option<f32>, b: Option<f32>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Ordena las impresiones por lo que se parece su ilustración a la de la
/// foto. Las de la misma ilustración se comparan una vez; las que pasan de
/// `MAX_ILUSTRACIONES` o no se pudieron bajar van al final sin distancia.
/// Dentro de la ilustración ganadora (si de verdad se parece), primero las
/// del `idioma` de la carta y, entre ellas, la de marco más parecido.
pub async fn ordenar_por_imagen(
    api: &impl CardApi,
    dir: &Path,
    foto: &Huellas,
    impresiones: Vec<ScryCard>,
    idioma: &str,
) -> Vec<(ScryCard, Option<f32>)> {
    // Grupos por ilustración, en el orden en que llegan.
    let mut grupos: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, c) in impresiones.iter().enumerate() {
        let clave = ilustracion(c);
        match grupos.iter_mut().find(|(k, _)| *k == clave) {
            Some((_, v)) => v.push(i),
            None => grupos.push((clave, vec![i])),
        }
    }
    // De cada grupo, la primera impresión con imagen.
    let representantes: Vec<Option<usize>> = grupos
        .iter()
        .take(MAX_ILUSTRACIONES)
        .map(|(_, v)| {
            v.iter()
                .copied()
                .find(|&i| url_pequena(&impresiones[i]).is_some())
        })
        .collect();
    let todas = &impresiones;
    let distancias: Vec<Option<f32>> = futures::stream::iter(representantes)
        .map(|r| async move {
            let c = &todas[r?];
            Some(foto.arte.distancia(&huellas_de(api, dir, c).await?.arte))
        })
        .buffered(A_LA_VEZ)
        .collect()
        .await;

    let mut por_impresion: Vec<Option<f32>> = vec![None; impresiones.len()];
    for ((_, v), d) in grupos.iter().zip(&distancias) {
        for &i in v {
            por_impresion[i] = *d;
        }
    }
    let mut v: Vec<(ScryCard, Option<f32>)> = impresiones.into_iter().zip(por_impresion).collect();
    // Las comparadas primero, de la más parecida a la menos; el resto, como
    // venían (orden estable).
    v.sort_by(|a, b| por_distancia(a.1, b.1));
    afinar_marco(api, dir, foto, &mut v, idioma).await;
    v
}

/// Entre las impresiones de la ilustración ganadora (las primeras de `v`),
/// las del idioma de la carta primero y, entre ellas, la de marco más
/// parecido a la foto: una por colección y número (los idiomas de una misma
/// impresión comparten marco).
async fn afinar_marco(
    api: &impl CardApi,
    dir: &Path,
    foto: &Huellas,
    v: &mut [(ScryCard, Option<f32>)],
    idioma: &str,
) {
    let Some((primera, Some(d))) = v.first() else {
        return;
    };
    if *d > UMBRAL_IMAGEN {
        return;
    }
    let ganadora = ilustracion(primera);
    let n = v
        .iter()
        .take_while(|(c, _)| ilustracion(c) == ganadora)
        .count();
    if n < 2 {
        return;
    }
    let bloque = &mut v[..n];
    let clave = |c: &ScryCard| (c.set.clone(), c.collector_number.clone());
    // Una impresión por colección y número, la del idioma si la hay.
    let mut elegidas: Vec<((String, String), usize)> = Vec::new();
    for (i, (c, _)) in bloque.iter().enumerate() {
        if url_pequena(c).is_none() {
            continue;
        }
        let k = clave(c);
        match elegidas.iter_mut().find(|(x, _)| *x == k) {
            Some((_, j)) if bloque[*j].0.lang != idioma && c.lang == idioma => *j = i,
            Some(_) => {}
            None => elegidas.push((k, i)),
        }
    }
    elegidas.truncate(MAX_MARCOS);
    let cartas = &*bloque;
    let marcos: Vec<((String, String), Option<f32>)> = futures::stream::iter(elegidas)
        .map(|(k, i)| async move {
            let h = huellas_de(api, dir, &cartas[i].0).await;
            (k, h.map(|h| foto.entera.distancia(&h.entera)))
        })
        .buffered(A_LA_VEZ)
        .collect()
        .await;
    let marco = |c: &ScryCard| {
        let k = clave(c);
        marcos.iter().find(|(x, _)| *x == k).and_then(|(_, d)| *d)
    };
    bloque.sort_by(|a, b| {
        (a.0.lang != idioma)
            .cmp(&(b.0.lang != idioma))
            .then_with(|| por_distancia(marco(&a.0), marco(&b.0)))
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::error::AppResult;
    use crate::scryfall::models::{Collection, Identifier, ImageUris};

    /// Una carta inventada con su ilustración (`semilla`) en la ventana de
    /// arte, de la medida que se pida.
    fn carta(semilla: u32, w: u32, h: u32) -> RgbaImage {
        let s = semilla as f32;
        RgbaImage::from_fn(w, h, |x, y| {
            let (u, v) = (x as f32 / w as f32 * 6.0, y as f32 / h as f32 * 6.0);
            let a = ((u * (2.0 + s)).sin() * (v * (1.0 + 0.5 * s)).cos() + 1.0) * 0.5;
            let b = ((u + v * (3.0 - 0.4 * s)).sin() + 1.0) * 0.5;
            image::Rgba([
                (40.0 + 200.0 * a) as u8,
                (30.0 + 190.0 * b) as u8,
                (90.0 * (a + b)) as u8,
                255,
            ])
        })
    }

    /// La ilustración 1 con un marco de color `n` alrededor (otra impresión
    /// del mismo arte).
    fn con_marco(n: u32, w: u32, h: u32) -> RgbaImage {
        let mut c = carta(1, w, h);
        let arte = proxy_core::foto::firma::ARTE;
        for (x, y, p) in c.enumerate_pixels_mut() {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            if !((arte.0..arte.2).contains(&u) && (arte.1..arte.3).contains(&v)) {
                *p = image::Rgba([(40 + 50 * n) as u8, 70, (200 - 40 * n) as u8, 255]);
            }
        }
        c
    }

    fn jpeg(img: &RgbaImage) -> Vec<u8> {
        proxy_core::imaging::encode::jpeg(img, 85).expect("jpeg")
    }

    fn impresion(id: &str, ilustracion: Option<&str>, url: Option<&str>) -> ScryCard {
        ScryCard {
            id: id.into(),
            name: "Prueba".into(),
            lang: "en".into(),
            illustration_id: ilustracion.map(String::from),
            image_uris: url.map(|u| ImageUris {
                small: Some(u.into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Scryfall de mentira: cada URL «img/<n>» es la ilustración `n`; cada
    /// «marco/<n>», la ilustración 1 con el marco `n`.
    struct Api {
        descargas: Mutex<Vec<String>>,
    }

    impl CardApi for Api {
        async fn collection(&self, _: &[Identifier]) -> AppResult<Collection> {
            Ok(Collection::default())
        }
        async fn card(&self, _: &str) -> AppResult<Option<ScryCard>> {
            Ok(None)
        }
        async fn card_lang(&self, _: &str, _: &str, _: &str) -> AppResult<Option<ScryCard>> {
            Ok(None)
        }
        async fn search(&self, _: &str, _: bool, _: usize) -> AppResult<Vec<ScryCard>> {
            Ok(Vec::new())
        }
        async fn download(&self, url: &str) -> AppResult<Vec<u8>> {
            self.descargas
                .lock()
                .expect("cerrojo")
                .push(url.to_string());
            if let Some(n) = url.strip_prefix("marco/") {
                return Ok(jpeg(&con_marco(n.parse().unwrap_or(0), 146, 204)));
            }
            let n: u32 = url.trim_start_matches("img/").parse().unwrap_or(0);
            Ok(jpeg(&carta(n, 146, 204)))
        }
    }

    fn falsa() -> Api {
        Api {
            descargas: Mutex::new(Vec::new()),
        }
    }

    #[tokio::test]
    async fn gana_la_ilustracion_de_la_foto_y_cada_una_se_baja_una_vez() {
        let dir = tempfile::tempdir().expect("tmp");
        let api = falsa();
        let impresiones = vec![
            impresion("a1", Some("ilu-1"), Some("img/1")),
            impresion("b1", Some("ilu-2"), Some("img/2")),
            impresion("b2", Some("ilu-2"), Some("img/2")),
            impresion("c1", Some("ilu-3"), Some("img/3")),
        ];
        // La foto: la ilustración 2, grande y algo más oscura.
        let mut foto = carta(2, 630, 880);
        for p in foto.pixels_mut() {
            for k in 0..3 {
                p[k] = (p[k] as f32 * 0.85) as u8;
            }
        }
        let v = ordenar_por_imagen(&api, dir.path(), &Huellas::de(&foto), impresiones, "en").await;
        let ids: Vec<&str> = v.iter().map(|(c, _)| c.id.as_str()).collect();
        assert_eq!(&ids[..2], &["b1", "b2"], "{ids:?}");
        assert!(v.iter().all(|(_, d)| d.is_some()));
        assert_eq!(api.descargas.lock().expect("cerrojo").len(), 3);

        // La segunda vez, de la caché: ninguna descarga más.
        let otra = falsa();
        let impresiones = vec![impresion("a1", Some("ilu-1"), Some("img/1"))];
        ordenar_por_imagen(&otra, dir.path(), &Huellas::de(&foto), impresiones, "en").await;
        assert!(otra.descargas.lock().expect("cerrojo").is_empty());
    }

    #[tokio::test]
    async fn sin_imagen_van_al_final_sin_distancia() {
        let dir = tempfile::tempdir().expect("tmp");
        let impresiones = vec![
            impresion("sin", Some("ilu-9"), None),
            impresion("con", Some("ilu-1"), Some("img/1")),
        ];
        let foto = Huellas::de(&carta(1, 630, 880));
        let v = ordenar_por_imagen(&falsa(), dir.path(), &foto, impresiones, "en").await;
        assert_eq!(v[0].0.id, "con");
        assert_eq!(v[1].0.id, "sin");
        assert!(v[1].1.is_none());
    }

    #[tokio::test]
    async fn solo_se_comparan_las_primeras_ilustraciones() {
        let dir = tempfile::tempdir().expect("tmp");
        let api = falsa();
        let impresiones: Vec<ScryCard> = (0..MAX_ILUSTRACIONES + 5)
            .map(|i| {
                impresion(
                    &format!("p{i}"),
                    Some(&format!("ilu-{i}")),
                    Some(&format!("img/{}", i % 7)),
                )
            })
            .collect();
        let foto = Huellas::de(&carta(1, 630, 880));
        let v = ordenar_por_imagen(&api, dir.path(), &foto, impresiones, "en").await;
        assert_eq!(
            v.iter().filter(|(_, d)| d.is_some()).count(),
            MAX_ILUSTRACIONES
        );
        assert_eq!(
            api.descargas.lock().expect("cerrojo").len(),
            MAX_ILUSTRACIONES
        );
    }

    fn en(mut c: ScryCard, set: &str, numero: &str, lang: &str) -> ScryCard {
        c.set = set.into();
        c.collector_number = numero.into();
        c.lang = lang.into();
        c
    }

    #[tokio::test]
    async fn entre_las_del_mismo_arte_gana_el_marco_de_la_foto() {
        let dir = tempfile::tempdir().expect("tmp");
        let impresiones = vec![
            en(
                impresion("m1", Some("ilu"), Some("marco/1")),
                "aaa",
                "1",
                "en",
            ),
            en(
                impresion("m2", Some("ilu"), Some("marco/2")),
                "bbb",
                "2",
                "en",
            ),
            en(
                impresion("m3", Some("ilu"), Some("marco/3")),
                "ccc",
                "3",
                "en",
            ),
        ];
        let foto = Huellas::de(&con_marco(2, 630, 880));
        let v = ordenar_por_imagen(&falsa(), dir.path(), &foto, impresiones, "en").await;
        assert_eq!(v[0].0.id, "m2");
    }

    #[tokio::test]
    async fn entre_las_del_mismo_arte_gana_el_idioma_de_la_carta() {
        let dir = tempfile::tempdir().expect("tmp");
        let impresiones = vec![
            en(
                impresion("ingles", Some("ilu"), Some("img/1")),
                "neo",
                "271",
                "en",
            ),
            en(
                impresion("espanol", Some("ilu"), Some("img/1")),
                "neo",
                "271",
                "es",
            ),
            en(
                impresion("otra", Some("otra"), Some("img/4")),
                "neo",
                "9",
                "es",
            ),
        ];
        let foto = Huellas::de(&carta(1, 630, 880));
        let v = ordenar_por_imagen(&falsa(), dir.path(), &foto, impresiones, "es").await;
        let ids: Vec<&str> = v.iter().map(|(c, _)| c.id.as_str()).collect();
        assert_eq!(ids, vec!["espanol", "ingles", "otra"]);
    }

    #[tokio::test]
    async fn si_ningun_arte_se_parece_no_se_baja_ningun_marco() {
        let dir = tempfile::tempdir().expect("tmp");
        let api = falsa();
        let impresiones = vec![
            en(
                impresion("x1", Some("ilu"), Some("img/5")),
                "aaa",
                "1",
                "en",
            ),
            en(
                impresion("x2", Some("ilu"), Some("img/5")),
                "bbb",
                "2",
                "en",
            ),
        ];
        // La foto es otra ilustración: nada se parece.
        let foto = Huellas::de(&carta(1, 630, 880));
        let v = ordenar_por_imagen(&api, dir.path(), &foto, impresiones, "en").await;
        assert!(v[0].1.is_some_and(|d| d > UMBRAL_IMAGEN), "{:?}", v[0].1);
        // Solo la del arte: el marco no se compara.
        assert_eq!(api.descargas.lock().expect("cerrojo").len(), 1);
    }

    #[test]
    fn un_id_raro_no_sale_de_la_carpeta() {
        assert_eq!(nombre_fichero("../../etc").as_deref(), Some("etc.jpg"));
        assert_eq!(nombre_fichero("0a1b-2c").as_deref(), Some("0a1b-2c.jpg"));
        assert_eq!(nombre_fichero("../"), None);
    }
}
