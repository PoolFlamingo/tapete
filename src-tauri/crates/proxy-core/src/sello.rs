//! **El sello de Tapete** («Tapete · No a la venta», en el idioma de la carta): que ninguna
//! carta que sale de la app pueda pasar por una oficial sin que se note (plan Tapete, Fase B).
//!
//! No se puede apagar. Va en lo que se imprime o se exporta: en el PDF como **texto vectorial
//! encima** de cada carta (así el JPEG de la carta sigue entrando tal cual, `passthrough`), y
//! **pintado en la imagen** en las miniaturas de las hojas y en las imágenes exportadas. Nunca en
//! la mesa ni en el editor.
//!
//! 🛑 Una sola cuenta para pantalla y papel: los glifos y su posición salen de [`linea`] (en
//! cuadratines) y la caja de [`caja`] (en fracciones de la carta); el PDF (`pdf::sello`) y
//! [`pintar`] dibujan lo mismo. Sin kerning a propósito: así no dependen de dos maquetadores.
//!
//! Dónde: centrado en la franja de abajo, **por debajo** de la línea del artista y del copyright
//! (en las cartas impresas queda ahí un borde libre de unos 2,5 mm). Si debajo hay un fondo liso
//! (el borde), el texto va solo, blanco o negro según el contraste; si hay ilustración (sin
//! borde, arte completo), sobre una pastilla oscura semitransparente.
//!
//! Letras: Montserrat (incrustada, a peso 600) para el latín y el cirílico; para japonés, coreano
//! y chino, recortes de Noto Sans JP/KR/SC/TC con solo los caracteres de los textos
//! (`assets/sello/`, los saca `scripts/sello-letras.py`).

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::OnceLock;

use image::RgbaImage;
use serde::Deserialize;
use skrifa::instance::{Location, LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::diseno::modelo::DesignLang;

pub(crate) const MONTSERRAT: &[u8] = include_bytes!("../assets/fuentes/Montserrat-Variable.ttf");
const LETRA_JA: &[u8] = include_bytes!("../assets/sello/sello-ja.ttf");
const LETRA_KO: &[u8] = include_bytes!("../assets/sello/sello-ko.ttf");
const LETRA_ZHS: &[u8] = include_bytes!("../assets/sello/sello-zhs.ttf");
const LETRA_ZHT: &[u8] = include_bytes!("../assets/sello/sello-zht.ttf");
const TEXTOS: &str = include_str!("../assets/sello/textos.json");

/// El peso de Montserrat (variable; su instancia de fábrica es la 100, demasiado fina para
/// 5 pt en papel). Los recortes CJK ya vienen a este peso.
pub const PESO: f32 = 600.0;

/// El cuadratín, en fracción del alto de la carta: en una de 88 mm, 1,80 mm ≈ 5,1 pt, como el
/// texto de copyright de las cartas impresas.
pub const TAM: f64 = 0.0205;
/// Relleno de la pastilla, en cuadratines.
const RELLENO_X: f64 = 0.45;
const SOBRE_BASE: f64 = 0.92;
const BAJO_BASE: f64 = 0.30;
/// Hasta dónde llega la tinta, en cuadratines: lo que se mira del fondo. Las mayúsculas latinas
/// y cirílicas llegan a ~0,72 y los ideogramas a ~0,88; lo que baja, a ~0,22.
const TINTA_SOBRE: f64 = 0.90;
const TINTA_BAJO: f64 = 0.24;
const TINTA_X: f64 = 0.10;

/// Dónde puede ir el sello: su escala (sobre [`TAM`]) y su línea base, en fracción del alto
/// desde el borde de abajo. En orden de preferencia: el de siempre y, si ahí hay algo (los
/// marcos propios llevan el pie más abajo que las cartas impresas), más abajo y algo más
/// pequeño, hasta 4,1 pt con lo que baja a ~0,5 mm del corte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sitio {
    pub escala: f64,
    pub base: f64,
    /// Pegado al margen derecho (en vez de centrado): para el pie alto de una carta diseñada
    /// cuya primera línea lleva texto en el centro.
    pub derecha: bool,
}

/// Hasta dónde llega por la derecha un sello pegado al margen (fracción del ancho): dentro de
/// las esquinas redondeadas.
const MARGEN_DERECHO: f64 = 0.875;

/// De dónde viene la carta: de una imagen propia (o de Scryfall), cuyo pie no se conoce, o del
/// editor de cartas, cuyo pie sí: la primera línea lleva solo el número y la rareza, a la
/// izquierda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    Propia,
    Diseno,
}

/// En una carta diseñada, el sello va en la primera línea del pie, a la altura del número y la
/// rareza («0016  R») y centrado: queda por encima del artista y del copyright (que van en la
/// segunda línea) y sin tocar la línea del marco. Se prueba antes que los de abajo y solo se
/// usa si ahí el fondo es liso (un pie propio con más texto lo descarta).
pub const PIE_ALTO: Sitio = Sitio {
    escala: 0.90,
    base: 0.0505,
    derecha: false,
};

/// El mismo sitio, pegado al margen derecho.
pub const PIE_ALTO_DERECHA: Sitio = Sitio {
    derecha: true,
    ..PIE_ALTO
};

pub const SITIOS: [Sitio; 4] = [
    Sitio {
        escala: 1.0,
        base: 0.0135,
        derecha: false,
    },
    Sitio {
        escala: 1.0,
        base: 0.0118,
        derecha: false,
    },
    Sitio {
        escala: 0.88,
        base: 0.0108,
        derecha: false,
    },
    Sitio {
        escala: 0.80,
        base: 0.0100,
        derecha: false,
    },
];
/// Opacidad de la pastilla oscura: con texto blanco da ≥ 4,5:1 sobre cualquier fondo.
pub const OPACIDAD_PASTILLA: f32 = 0.62;
/// Hasta esta desviación de la luminancia (0–255) el fondo cuenta como liso.
const LISO: f64 = 14.0;
/// Un píxel que se aparta tanto de la media (luminancia 0–255) es un trazo, no fondo.
const TRAZO: f64 = 28.0;
/// En una carta diseñada (de la que se espera un borde limpio) no se admite casi ningún trazo
/// dentro de la caja: así el sello se aparta de las colas de las letras del pie.
const TRAZOS_MAXIMOS: f64 = 0.0004;

#[derive(Deserialize, Default)]
struct Textos {
    carta: BTreeMap<String, String>,
    hoja: BTreeMap<String, String>,
}

fn textos() -> &'static Textos {
    static T: OnceLock<Textos> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(TEXTOS).unwrap_or_default())
}

fn de_tabla<'a>(t: &'a BTreeMap<String, String>, lang: DesignLang, respaldo: &'a str) -> &'a str {
    t.get(lang.codigo())
        .or_else(|| t.get("en"))
        .map_or(respaldo, String::as_str)
}

/// El texto del sello de una carta.
pub fn texto_carta(lang: DesignLang) -> &'static str {
    de_tabla(&textos().carta, lang, "Tapete · Not for sale")
}

/// La línea del margen de cada hoja del PDF (en el idioma de la app).
pub fn texto_hoja(lang: DesignLang) -> &'static str {
    de_tabla(&textos().hoja, lang, "Made with Tapete · Not for sale")
}

/// El idioma del sello a partir de un código de Scryfall o de la interfaz; lo que no es uno de
/// los 15 (latín, hebreo, pirexiano…), en inglés.
pub fn idioma(codigo: Option<&str>) -> DesignLang {
    codigo
        .and_then(DesignLang::de_codigo)
        .unwrap_or(DesignLang::En)
}

/// El idioma del sello de una carta: el de su diseño si es diseñada; si no, el de la imagen
/// (las propias lo dicen aparte) o el de la impresión; si no se sabe, el de diseño del deck.
pub fn idioma_de_carta(c: &crate::model::DeckCard, lang_deck: DesignLang) -> DesignLang {
    if c.design.is_some() {
        return lang_deck;
    }
    c.meta
        .image_lang
        .as_deref()
        .or(c.meta.lang.as_deref())
        .map_or(lang_deck, |codigo| idioma(Some(codigo)))
}

/// La letra de un idioma: sus bytes y si es la variable (Montserrat).
pub fn letra(lang: DesignLang) -> (&'static [u8], bool) {
    match lang {
        DesignLang::Ja => (LETRA_JA, false),
        DesignLang::Ko => (LETRA_KO, false),
        DesignLang::Zhs => (LETRA_ZHS, false),
        DesignLang::Zht => (LETRA_ZHT, false),
        _ => (MONTSERRAT, true),
    }
}

/// Un glifo ya colocado: su id, dónde empieza (en cuadratines desde el principio de la línea),
/// cuánto avanza y qué trozo del texto es (para que el PDF se pueda buscar y copiar).
#[derive(Debug, Clone, PartialEq)]
pub struct Glifo {
    pub id: u32,
    pub x: f32,
    pub avance: f32,
    pub rango: Range<usize>,
}

/// Una línea del sello: el texto, su letra y sus glifos.
#[derive(Debug, Clone)]
pub struct Linea {
    pub texto: &'static str,
    pub lang: DesignLang,
    pub glifos: Vec<Glifo>,
    /// Ancho total, en cuadratines.
    pub ancho: f32,
}

fn ubicacion(fuente: &FontRef, variable: bool) -> Location {
    if variable {
        fuente.axes().location([("wght", PESO)])
    } else {
        Location::default()
    }
}

/// Coloca un texto con la letra de su idioma: cada carácter, su glifo y su avance.
pub fn linea(texto: &'static str, lang: DesignLang) -> Linea {
    let (datos, variable) = letra(lang);
    let mut glifos = Vec::new();
    let mut x = 0.0f32;
    if let Ok(fuente) = FontRef::new(datos) {
        let loc = ubicacion(&fuente, variable);
        let upem = f32::from(
            fuente
                .metrics(Size::unscaled(), LocationRef::from(&loc))
                .units_per_em
                .max(1),
        );
        let metricas = fuente.glyph_metrics(Size::unscaled(), LocationRef::from(&loc));
        let mapa = fuente.charmap();
        for (i, c) in texto.char_indices() {
            let id = mapa.map(c).unwrap_or(GlyphId::NOTDEF);
            let avance = metricas.advance_width(id).unwrap_or(0.0) / upem;
            glifos.push(Glifo {
                id: id.to_u32(),
                x,
                avance,
                rango: i..i + c.len_utf8(),
            });
            x += avance;
        }
    }
    Linea {
        texto,
        lang,
        glifos,
        ancho: x,
    }
}

/// La línea del sello de una carta.
pub fn linea_carta(lang: DesignLang) -> Linea {
    linea(texto_carta(lang), lang)
}

/// Dónde va el sello en una carta de `ancho` × `alto` (en las unidades que se quieran: px,
/// mm o pt): el cuadratín, el comienzo del texto, la línea base, la caja de la pastilla
/// (x0, y0, x1, y1) y la de la tinta (lo que se mira del fondo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caja {
    pub em: f64,
    pub x: f64,
    pub base: f64,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub tinta: [f64; 4],
}

pub fn caja(l: &Linea, ancho: f64, alto: f64, sitio: Sitio) -> Caja {
    let em = TAM * sitio.escala * alto;
    let w = f64::from(l.ancho) * em;
    let x = if sitio.derecha {
        ancho * MARGEN_DERECHO - w - RELLENO_X * em
    } else {
        (ancho - w) / 2.0
    };
    let base = alto - sitio.base * alto;
    Caja {
        em,
        x,
        base,
        x0: x - RELLENO_X * em,
        y0: base - SOBRE_BASE * em,
        x1: x + w + RELLENO_X * em,
        y1: base + BAJO_BASE * em,
        tinta: [
            x - TINTA_X * em,
            base - TINTA_SOBRE * em,
            x + w + TINTA_X * em,
            base + TINTA_BAJO * em,
        ],
    }
}

/// Cómo va el fondo del sello.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fondo {
    /// Sobre un fondo liso (el borde de la carta): solo el texto, blanco o negro.
    Banda { claro: bool },
    /// Sobre la ilustración: texto blanco sobre una pastilla oscura semitransparente.
    Pastilla,
}

/// Cómo y dónde se pinta el sello en una carta concreta (lo decide [`colocar`] mirando la
/// imagen; el PDF lo usa tal cual).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colocacion {
    pub sitio: Sitio,
    pub fondo: Fondo,
}

impl Colocacion {
    /// El color del texto (r, g, b).
    pub fn color(self) -> (u8, u8, u8) {
        match self.fondo {
            Fondo::Banda { claro: false } => (0, 0, 0),
            _ => (255, 255, 255),
        }
    }
}

fn lineal(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Desviación de la luminancia (0–255), luminancia relativa media y qué parte de los píxeles se
/// aparta mucho de la media (un trazo fino, como la cola de una letra, casi no mueve la
/// desviación pero sí aparece aquí) de un rectángulo.
fn fondo(img: &RgbaImage, r: [f64; 4]) -> Option<(f64, f64, f64)> {
    let (w, h) = img.dimensions();
    let x0 = r[0].max(0.0).floor() as u32;
    let y0 = r[1].max(0.0).floor() as u32;
    let x1 = (r[2].ceil().max(0.0) as u32).min(w);
    let y1 = (r[3].ceil().max(0.0) as u32).min(h);
    let (mut n, mut suma, mut suma2, mut lum) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut lumas: Vec<f64> = Vec::new();
    // Lo transparente (las esquinas redondeadas) se ve sobre el papel blanco.
    for y in y0..y1 {
        for x in x0..x1 {
            let p = img.get_pixel(x, y).0;
            let a = f64::from(p[3]) / 255.0;
            let rgb = [0, 1, 2].map(|i| (f64::from(p[i]) * a + 255.0 * (1.0 - a)).round() as u8);
            let luma =
                0.299 * f64::from(rgb[0]) + 0.587 * f64::from(rgb[1]) + 0.114 * f64::from(rgb[2]);
            lumas.push(luma);
            suma += luma;
            suma2 += luma * luma;
            lum += 0.2126 * lineal(rgb[0]) + 0.7152 * lineal(rgb[1]) + 0.0722 * lineal(rgb[2]);
            n += 1.0;
        }
    }
    if n == 0.0 {
        return None;
    }
    let media = suma / n;
    let lejos = lumas.iter().filter(|l| (**l - media).abs() > TRAZO).count();
    Some((
        (suma2 / n - media * media).max(0.0).sqrt(),
        lum / n,
        lejos as f64 / n,
    ))
}

/// Mira el fondo de la carta ya recortada (sin sangrado) en cada sitio posible y decide: el
/// primero liso, sin pastilla y con el color de más contraste; si ninguno lo es, el que menos
/// se mueve, con pastilla.
/// El ancho al que se mira la carta antes de decidir: igual para la miniatura (146–745 px) que
/// para el PDF (300–900 PPP), así las dos eligen lo mismo aunque una vea más detalle.
const ANCHO_MIRADA: u32 = 400;

pub fn colocar(img: &RgbaImage, l: &Linea) -> Colocacion {
    colocar_como(img, l, Origen::Propia)
}

/// Como [`colocar`], sabiendo de dónde viene la carta: una diseñada prueba antes [`PIE_ALTO`].
pub fn colocar_como(img: &RgbaImage, l: &Linea, origen: Origen) -> Colocacion {
    let (w0, h0) = img.dimensions();
    let reducida;
    let img = if w0 == ANCHO_MIRADA || w0 == 0 {
        img
    } else {
        let h = ((f64::from(h0) * f64::from(ANCHO_MIRADA) / f64::from(w0)).round() as u32).max(1);
        reducida = image::imageops::resize(img, ANCHO_MIRADA, h, image::imageops::Triangle);
        &reducida
    };
    let (w, h) = img.dimensions();
    let mut mejor: Option<(f64, Sitio)> = None;
    // Una imagen propia: los sitios de siempre. Una diseñada: el pie alto y, para cada sitio,
    // también pegado a la derecha (el artista suele ocupar el centro-izquierda del pie y la
    // derecha queda libre).
    let candidatos: Vec<Sitio> = match origen {
        Origen::Propia => SITIOS.to_vec(),
        Origen::Diseno => [PIE_ALTO]
            .into_iter()
            .chain(SITIOS)
            .flat_map(|s| [s, Sitio { derecha: true, ..s }])
            .collect(),
    };
    for sitio in candidatos {
        let c = caja(l, f64::from(w), f64::from(h), sitio);
        let Some((desviacion, y, trazos)) = fondo(img, c.tinta) else {
            continue;
        };
        let limpio = origen == Origen::Propia || trazos <= TRAZOS_MAXIMOS;
        if desviacion <= LISO && limpio {
            // Con blanco o negro, el mejor de los dos da siempre ≥ 4,58:1 (WCAG AA pide 4,5)
            // sobre un fondo liso.
            let con_blanco = 1.05 / (y + 0.05);
            let con_negro = (y + 0.05) / 0.05;
            return Colocacion {
                sitio,
                fondo: Fondo::Banda {
                    claro: con_blanco >= con_negro,
                },
            };
        }
        // Sobre la ilustración, la pastilla va donde ya iba: abajo y centrada.
        if SITIOS.contains(&sitio) && mejor.is_none_or(|(d, _)| desviacion < d) {
            mejor = Some((desviacion, sitio));
        }
    }
    Colocacion {
        sitio: mejor.map_or(SITIOS[0], |(_, s)| s),
        fondo: Fondo::Pastilla,
    }
}

/// Dónde va el sello de una imagen ya preparada para el PDF (con su sangrado alrededor), para
/// cada idioma pedido: la decodifica, le quita el sangrado y la mira con [`colocar`].
pub fn colocaciones(
    bytes: &[u8],
    ancho_mm: f64,
    sangrado_mm: f64,
    langs: &[DesignLang],
    origen: Origen,
) -> Result<Vec<(DesignLang, Colocacion)>, crate::imaging::ImagingError> {
    let img = crate::imaging::decode::decode(bytes)?;
    let (w, h) = img.dimensions();
    let px_mm = f64::from(w) / (ancho_mm + 2.0 * sangrado_mm).max(f64::EPSILON);
    let b = ((sangrado_mm * px_mm).round().max(0.0) as u32)
        .min(w / 4)
        .min(h / 4);
    let carta = image::imageops::crop_imm(&img, b, b, w - 2 * b, h - 2 * b).to_image();
    Ok(langs
        .iter()
        .map(|&lang| (lang, colocar_como(&carta, &linea_carta(lang), origen)))
        .collect())
}

/// Un trazo de skrifa a un camino de tiny-skia, ya en píxeles de la carta.
struct Lapiz<'a> {
    pb: &'a mut PathBuilder,
    dx: f32,
    base: f32,
}

impl OutlinePen for Lapiz<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.dx + x, self.base - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.dx + x, self.base - y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb
            .quad_to(self.dx + cx0, self.base - cy0, self.dx + x, self.base - y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.dx + cx0,
            self.base - cy0,
            self.dx + cx1,
            self.base - cy1,
            self.dx + x,
            self.base - y,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Un trazo de un camino, para dibujarlo igual con tiny-skia (la imagen) y con krilla (el PDF).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trazo {
    Mover(f32, f32),
    Linea(f32, f32),
    Cubica(f32, f32, f32, f32, f32, f32),
    Cerrar,
}

/// Una pastilla (rectángulo de puntas redondas).
pub fn pastilla(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Trazo> {
    let r = ((y1 - y0) / 2.0).min((x1 - x0) / 2.0);
    // κ: los cuartos de círculo con cúbicas.
    let k = 0.552_284_8 * r;
    vec![
        Trazo::Mover(x0 + r, y0),
        Trazo::Linea(x1 - r, y0),
        Trazo::Cubica(x1 - r + k, y0, x1, y0 + r - k, x1, y0 + r),
        Trazo::Cubica(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1),
        Trazo::Linea(x0 + r, y1),
        Trazo::Cubica(x0 + r - k, y1, x0, y1 - r + k, x0, y1 - r),
        Trazo::Cubica(x0, y0 + r - k, x0 + r - k, y0, x0 + r, y0),
        Trazo::Cerrar,
    ]
}

/// Pinta el sello en una carta ya recortada a su medida (sin sangrado) y dice cómo lo puso.
pub fn pintar(img: &mut RgbaImage, lang: DesignLang) -> Colocacion {
    pintar_como(img, lang, Origen::Propia)
}

/// Como [`pintar`], sabiendo de dónde viene la carta (ver [`colocar_como`]).
pub fn pintar_como(img: &mut RgbaImage, lang: DesignLang, origen: Origen) -> Colocacion {
    let l = linea_carta(lang);
    let colocacion = colocar_como(img, &l, origen);
    let (w, h) = img.dimensions();
    let c = caja(&l, f64::from(w), f64::from(h), colocacion.sitio);
    // Solo se pinta la caja: un lienzo pequeño que luego se funde sobre la carta.
    let ox = c.x0.max(0.0).floor();
    let oy = c.y0.max(0.0).floor();
    let pw = ((c.x1.ceil() - ox) as u32)
        .min(w.saturating_sub(ox as u32))
        .max(1);
    let ph = ((c.y1.ceil() - oy) as u32)
        .min(h.saturating_sub(oy as u32))
        .max(1);
    let Some(mut lienzo) = Pixmap::new(pw, ph) else {
        return colocacion;
    };
    let mover = |v: f64, o: f64| (v - o) as f32;

    if colocacion.fondo == Fondo::Pastilla {
        let mut pb = PathBuilder::new();
        for t in pastilla(
            mover(c.x0, ox),
            mover(c.y0, oy),
            mover(c.x1, ox),
            mover(c.y1, oy),
        ) {
            match t {
                Trazo::Mover(x, y) => pb.move_to(x, y),
                Trazo::Linea(x, y) => pb.line_to(x, y),
                Trazo::Cubica(a, b, c, d, x, y) => pb.cubic_to(a, b, c, d, x, y),
                Trazo::Cerrar => pb.close(),
            }
        }
        if let Some(camino) = pb.finish() {
            let mut p = Paint::default();
            p.set_color_rgba8(0, 0, 0, (OPACIDAD_PASTILLA * 255.0).round() as u8);
            p.anti_alias = true;
            lienzo.fill_path(&camino, &p, FillRule::Winding, Transform::identity(), None);
        }
    }

    let (datos, variable) = letra(lang);
    if let Ok(fuente) = FontRef::new(datos) {
        let loc = ubicacion(&fuente, variable);
        let contornos = fuente.outline_glyphs();
        let mut pb = PathBuilder::new();
        let tam = c.em as f32;
        for g in &l.glifos {
            let Some(contorno) = contornos.get(GlyphId::new(g.id)) else {
                continue;
            };
            let mut lapiz = Lapiz {
                pb: &mut pb,
                dx: mover(c.x, ox) + g.x * tam,
                base: mover(c.base, oy),
            };
            let ajustes = DrawSettings::unhinted(Size::new(tam), LocationRef::from(&loc));
            let _ = contorno.draw(ajustes, &mut lapiz);
        }
        if let Some(camino) = pb.finish() {
            let (r, g, b) = colocacion.color();
            let mut p = Paint::default();
            p.set_color_rgba8(r, g, b, 255);
            p.anti_alias = true;
            lienzo.fill_path(&camino, &p, FillRule::Winding, Transform::identity(), None);
        }
    }

    // Fundir (el lienzo va premultiplicado; la carta, no).
    let (ox, oy) = (ox as u32, oy as u32);
    for y in 0..ph {
        for x in 0..pw {
            let Some(s) = lienzo.pixel(x, y) else {
                continue;
            };
            let a = f32::from(s.alpha()) / 255.0;
            if a == 0.0 {
                continue;
            }
            let d = img.get_pixel_mut(ox + x, oy + y);
            let [dr, dg, db, da] = d.0;
            let fa = f32::from(da) / 255.0;
            let sale_a = a + fa * (1.0 - a);
            let mezcla = |s: u8, d: u8| {
                let v = (f32::from(s) + f32::from(d) * fa * (1.0 - a)) / sale_a.max(f32::EPSILON);
                v.round().clamp(0.0, 255.0) as u8
            };
            d.0 = [
                mezcla(s.red(), dr),
                mezcla(s.green(), dg),
                mezcla(s.blue(), db),
                (sale_a * 255.0).round() as u8,
            ];
        }
    }
    colocacion
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn carta(color: [u8; 3]) -> RgbaImage {
        RgbaImage::from_pixel(630, 880, Rgba([color[0], color[1], color[2], 255]))
    }

    #[test]
    fn los_quince_idiomas_tienen_texto_y_todos_sus_glifos() {
        for lang in DesignLang::TODOS {
            for t in [texto_carta(lang), texto_hoja(lang)] {
                assert!(t.contains("Tapete"), "{lang:?}: {t}");
                let l = linea(t, lang);
                assert_eq!(l.glifos.len(), t.chars().count());
                for (g, c) in l.glifos.iter().zip(t.chars()) {
                    assert_ne!(g.id, 0, "{lang:?}: «{c}» sin glifo en su letra");
                    assert!(g.avance > 0.0 || c == ' ', "{lang:?}: «{c}» sin avance");
                }
            }
        }
    }

    #[test]
    fn cabe_en_una_carta_sin_estrecharse_y_lejos_del_corte() {
        for lang in DesignLang::TODOS {
            let l = linea_carta(lang);
            for sitio in SITIOS {
                let c = caja(&l, 63.0, 88.0, sitio);
                // Entera dentro de la carta, con margen a los lados (las esquinas redondeadas).
                assert!(c.x0 > 63.0 * 0.12, "{lang:?} {sitio:?}: {c:?}");
                assert!(c.x1 < 63.0 * 0.88, "{lang:?} {sitio:?}: {c:?}");
                // Lo que baja, a ~0,5 mm del corte o más; lo más alto, por debajo de la línea
                // del artista (a ~3,3 % del alto desde abajo en las cartas impresas).
                assert!(88.0 - c.tinta[3] >= 0.5, "{lang:?} {sitio:?}: {c:?}");
                assert!(
                    88.0 - c.tinta[1] < 88.0 * 0.033,
                    "{lang:?} {sitio:?}: {c:?}"
                );
            }
            for sitio in SITIOS {
                let c = caja(
                    &l,
                    63.0,
                    88.0,
                    Sitio {
                        derecha: true,
                        ..sitio
                    },
                );
                assert!(
                    c.x0 > 63.0 * 0.12 && c.x1 < 63.0 * 0.88,
                    "{lang:?} {sitio:?}: {c:?}"
                );
            }
            // El tamaño, nunca por debajo de ~4 pt (1,41 mm de cuadratín).
            for sitio in SITIOS {
                assert!(caja(&l, 63.0, 88.0, sitio).em > 1.4);
            }
        }
    }

    #[test]
    fn el_sitio_del_pie_alto_cabe_entre_el_marco_y_el_artista() {
        for lang in DesignLang::TODOS {
            let c = caja(&linea_carta(lang), 63.0, 88.0, PIE_ALTO);
            assert!(c.x0 > 63.0 * 0.12 && c.x1 < 63.0 * 0.88, "{lang:?}: {c:?}");
            assert!(c.em > 1.4, "{lang:?}: {c:?}");
            let d = caja(&linea_carta(lang), 63.0, 88.0, PIE_ALTO_DERECHA);
            assert!(d.x0 > 63.0 * 0.12 && d.x1 < 63.0 * 0.88, "{lang:?}: {d:?}");
            // Por encima de la línea del artista y del copyright (3,3 % desde abajo) y por
            // debajo de la línea del marco (~7,1 %).
            assert!(88.0 - c.tinta[3] > 88.0 * 0.033, "{lang:?}: {c:?}");
            assert!(88.0 - c.tinta[1] < 88.0 * 0.070, "{lang:?}: {c:?}");
        }
    }

    #[test]
    fn una_carta_disenada_lo_lleva_en_la_primera_linea_del_pie_y_una_propia_abajo() {
        let l = linea_carta(DesignLang::Es);
        let img = carta([10, 10, 12]);
        assert_eq!(colocar_como(&img, &l, Origen::Diseno).sitio, PIE_ALTO);
        // Sin saber de dónde viene, como siempre.
        assert_eq!(colocar(&img, &l).sitio, SITIOS[0]);
        assert_eq!(colocar_como(&img, &l, Origen::Propia).sitio, SITIOS[0]);
    }

    #[test]
    fn si_el_centro_de_la_primera_linea_esta_ocupado_va_a_la_derecha() {
        let l = linea_carta(DesignLang::Es);
        let mut img = carta([0, 0, 0]);
        let c = caja(&l, 630.0, 880.0, PIE_ALTO);
        let y = ((c.tinta[1] + c.tinta[3]) / 2.0) as u32;
        // Texto de la primera columna al centro-izquierda, como el artista del pie de fábrica.
        for x in 150..330 {
            img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
        }
        let col = colocar_como(&img, &l, Origen::Diseno);
        assert_eq!(col.sitio, PIE_ALTO_DERECHA);
        assert_eq!(col.fondo, Fondo::Banda { claro: true });
    }

    #[test]
    fn si_la_primera_linea_del_pie_lleva_texto_en_el_centro_baja_al_sitio_de_siempre() {
        let l = linea_carta(DesignLang::Es);
        let mut img = carta([0, 0, 0]);
        let c = caja(&l, 630.0, 880.0, PIE_ALTO);
        let y = ((c.tinta[1] + c.tinta[3]) / 2.0) as u32;
        for x in 100..560 {
            img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
        }
        let col = colocar_como(&img, &l, Origen::Diseno);
        assert_eq!(col.sitio, SITIOS[0]);
        assert_eq!(col.fondo, Fondo::Banda { claro: true });
    }

    #[test]
    fn con_el_artista_en_el_centro_de_las_dos_lineas_va_abajo_a_la_derecha() {
        // Como el pie de fábrica del editor: texto en el centro-izquierda de las dos líneas y
        // la derecha libre (menos la caja de fuerza/resistencia, que llega a la línea alta).
        let l = linea_carta(DesignLang::Es);
        let mut img = carta([0, 0, 0]);
        let alto = caja(&l, 630.0, 880.0, PIE_ALTO);
        let bajo = caja(&l, 630.0, 880.0, SITIOS[0]);
        for c in [alto, bajo] {
            let y = ((c.tinta[1] + c.tinta[3]) / 2.0) as u32;
            for x in 120..300 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        for x in 400..560 {
            img.put_pixel(x, alto.tinta[3] as u32, image::Rgba([255, 255, 255, 255]));
        }
        let col = colocar_como(&img, &l, Origen::Diseno);
        assert_eq!(
            col.sitio,
            Sitio {
                derecha: true,
                ..SITIOS[0]
            }
        );
        assert_eq!(col.fondo, Fondo::Banda { claro: true });
    }

    #[test]
    fn una_carta_disenada_se_aparta_de_las_colas_de_las_letras_del_pie() {
        // Una cola fina que entra en la caja de siempre por arriba: apenas mueve la desviación
        // media (una imagen propia no la tiene en cuenta), pero en una diseñada cuenta.
        let l = linea_carta(DesignLang::Es);
        let mut img = carta([0, 0, 0]);
        let c = caja(&l, 630.0, 880.0, SITIOS[0]);
        let y = c.tinta[1].ceil() as u32 + 1;
        for x in 250..256 {
            img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            img.put_pixel(x, y + 1, image::Rgba([255, 255, 255, 255]));
        }
        // Y el pie alto, ocupado, para que se llegue a los de abajo.
        let alto = caja(&l, 630.0, 880.0, PIE_ALTO);
        let ya = ((alto.tinta[1] + alto.tinta[3]) / 2.0) as u32;
        for x in 100..560 {
            img.put_pixel(x, ya, image::Rgba([255, 255, 255, 255]));
        }
        assert_eq!(colocar_como(&img, &l, Origen::Propia).sitio, SITIOS[0]);
        let col = colocar_como(&img, &l, Origen::Diseno);
        assert_ne!(col.sitio, SITIOS[0]);
        assert_eq!(col.fondo, Fondo::Banda { claro: true });
    }

    #[test]
    fn sobre_la_ilustracion_una_carta_disenada_no_pone_la_pastilla_en_el_pie_alto() {
        let mut img = carta([20, 20, 20]);
        for (x, y, p) in img.enumerate_pixels_mut() {
            p.0 = [((x * 7) % 256) as u8, ((y * 13) % 256) as u8, 90, 255];
        }
        let col = colocar_como(&img, &linea_carta(DesignLang::En), Origen::Diseno);
        assert_eq!(col.fondo, Fondo::Pastilla);
        assert_ne!(col.sitio, PIE_ALTO);
    }

    #[test]
    fn el_idioma_de_una_carta() {
        assert_eq!(idioma(Some("ja")), DesignLang::Ja);
        assert_eq!(idioma(Some("ph")), DesignLang::En);
        assert_eq!(idioma(None), DesignLang::En);
    }

    #[test]
    fn sobre_un_borde_liso_va_solo_y_con_contraste() {
        let l = linea_carta(DesignLang::Es);
        let banda = |claro| Colocacion {
            sitio: SITIOS[0],
            fondo: Fondo::Banda { claro },
        };
        assert_eq!(colocar(&carta([10, 10, 12]), &l), banda(true));
        assert_eq!(colocar(&carta([245, 245, 240]), &l), banda(false));
        // Hasta el gris más incómodo tiene 4,5:1 con uno de los dos.
        for gris in [100u8, 118, 120, 135] {
            assert!(matches!(
                colocar(&carta([gris; 3]), &l).fondo,
                Fondo::Banda { .. }
            ));
        }
    }

    #[test]
    fn sobre_la_ilustracion_va_en_pastilla() {
        let mut img = carta([20, 20, 20]);
        for (x, y, p) in img.enumerate_pixels_mut() {
            p.0 = [((x * 7) % 256) as u8, ((y * 13) % 256) as u8, 90, 255];
        }
        assert_eq!(
            colocar(&img, &linea_carta(DesignLang::En)).fondo,
            Fondo::Pastilla
        );
    }

    #[test]
    fn si_el_pie_llega_abajo_busca_un_sitio_mas_bajo() {
        // Un borde negro con una raya de texto blanco justo donde iría el sello de siempre.
        let l = linea_carta(DesignLang::Es);
        let mut img = carta([0, 0, 0]);
        let c = caja(&l, 630.0, 880.0, SITIOS[0]);
        let raya = c.tinta[1].ceil() as u32 + 1;
        for x in 100..530 {
            img.put_pixel(x, raya, image::Rgba([255, 255, 255, 255]));
        }
        let col = colocar(&img, &l);
        assert_ne!(col.sitio, SITIOS[0]);
        assert_eq!(col.fondo, Fondo::Banda { claro: true });
        // Y el sitio elegido no toca la raya.
        let c = caja(&l, 630.0, 880.0, col.sitio);
        assert!(c.tinta[1] > f64::from(raya));
    }

    #[test]
    fn pintar_solo_cambia_la_franja_del_sello() {
        for lang in [DesignLang::Es, DesignLang::Ja, DesignLang::Ru] {
            let antes = carta([12, 12, 12]);
            let mut img = antes.clone();
            let col = pintar(&mut img, lang);
            assert_eq!(col.fondo, Fondo::Banda { claro: true });
            let c = caja(&linea_carta(lang), 630.0, 880.0, col.sitio);
            let mut cambiados = 0;
            for (x, y, p) in img.enumerate_pixels() {
                if p != antes.get_pixel(x, y) {
                    cambiados += 1;
                    let (x, y) = (f64::from(x), f64::from(y));
                    assert!(x >= c.x0.floor() && x <= c.x1.ceil(), "{lang:?} x={x}");
                    assert!(y >= c.y0.floor() && y <= c.y1.ceil(), "{lang:?} y={y}");
                }
            }
            assert!(cambiados > 200, "{lang:?}: casi nada pintado ({cambiados})");
        }
    }

    #[test]
    fn la_colocacion_de_una_imagen_con_sangrado_es_la_de_la_carta_sin_el() {
        // Borde negro liso y un sangrado blanco alrededor: sin quitarlo, la caja tocaría lo
        // blanco de abajo y daría otra cosa.
        let carta_px = carta([5, 5, 5]);
        let b = 30u32;
        let mut con = RgbaImage::from_pixel(630 + 2 * b, 880 + 2 * b, image::Rgba([255; 4]));
        image::imageops::overlay(&mut con, &carta_px, i64::from(b), i64::from(b));
        let png = crate::imaging::encode::png(&con).unwrap();
        // 630 px = 63 mm → 10 px/mm → 3 mm de sangrado.
        let r = colocaciones(
            &png,
            63.0,
            3.0,
            &[DesignLang::Es, DesignLang::Ko],
            Origen::Propia,
        )
        .unwrap();
        for (lang, c) in r {
            assert_eq!(c, colocar(&carta_px, &linea_carta(lang)), "{lang:?}");
        }
    }

    /// La miniatura y el PDF ven la misma carta a tamaños muy distintos: tienen que elegir lo
    /// mismo (plan B, «una sola geometría»).
    #[test]
    fn la_miniatura_y_el_pdf_eligen_lo_mismo() {
        let mut grande = RgbaImage::from_pixel(1488, 2079, image::Rgba([8, 8, 8, 255]));
        // Una línea de pie fina, justo donde iría el sello de siempre.
        let l = linea_carta(DesignLang::Es);
        let c = caja(&l, 1488.0, 2079.0, SITIOS[0]);
        for y in (c.tinta[1] as u32 + 2)..(c.tinta[1] as u32 + 6) {
            for x in 300..1200 {
                grande.put_pixel(x, y, image::Rgba([230, 230, 230, 255]));
            }
        }
        let esperado = colocar(&grande, &l);
        for ancho in [146u32, 244, 488, 745] {
            let alto = (2079.0 * f64::from(ancho) / 1488.0).round() as u32;
            let mini = image::imageops::resize(&grande, ancho, alto, image::imageops::Triangle);
            assert_eq!(colocar(&mini, &l), esperado, "{ancho}");
        }
    }

    #[test]
    fn dos_veces_lo_mismo_da_los_mismos_pixeles() {
        let mut a = carta([200, 30, 30]);
        let mut b = a.clone();
        pintar(&mut a, DesignLang::Zht);
        pintar(&mut b, DesignLang::Zht);
        assert_eq!(a, b);
    }
}
