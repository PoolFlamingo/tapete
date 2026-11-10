//! **El sello en el PDF**: texto vectorial encima de cada carta (así la imagen de la carta
//! sigue entrando tal cual) y la línea del margen de cada hoja. Los glifos, su posición y la
//! caja salen de `crate::sello`, lo mismo que pinta la miniatura.

use std::collections::BTreeMap;

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point};
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, FillRule};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph, Tag};
use krilla::Data;

use crate::diseno::modelo::DesignLang;
use crate::layout::{Page, RectMm};
use crate::sello::{self, Colocacion, Fondo, Linea, Trazo};
use crate::units::mm_to_pt;

/// El sello de una casilla: su idioma y cómo va (lo decidió `sello::colocar` mirando la
/// imagen de la carta).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelloCasilla {
    pub lang: DesignLang,
    pub colocacion: Colocacion,
}

/// El tamaño de la línea del margen (pt) y el hueco mínimo que necesita (mm).
const TAM_HOJA_PT: f32 = 6.0;
const HUECO_HOJA_MM: f64 = 3.2;
const GRIS_HOJA: u8 = 120;

fn pt(mm: f64) -> f32 {
    mm_to_pt(mm) as f32
}

/// Las letras del documento, una por idioma (krilla las recorta y las incrusta una vez).
#[derive(Default)]
pub(crate) struct Letras(BTreeMap<DesignLang, Option<Font>>);

impl Letras {
    fn de(&mut self, lang: DesignLang) -> Option<Font> {
        self.0
            .entry(lang)
            .or_insert_with(|| {
                let (datos, variable) = sello::letra(lang);
                let datos = Data::from(datos);
                if variable {
                    Font::new_variable(datos, 0, &[(Tag::new(b"wght"), sello::PESO)])
                } else {
                    Font::new(datos, 0)
                }
            })
            .clone()
    }
}

fn camino(trazos: &[Trazo]) -> Option<krilla::geom::Path> {
    let mut pb = PathBuilder::new();
    for t in trazos {
        match *t {
            Trazo::Mover(x, y) => pb.move_to(x, y),
            Trazo::Linea(x, y) => pb.line_to(x, y),
            Trazo::Cubica(a, b, c, d, x, y) => pb.cubic_to(a, b, c, d, x, y),
            Trazo::Cerrar => pb.close(),
        }
    }
    pb.finish()
}

fn relleno(r: u8, g: u8, b: u8, opacidad: f32) -> Fill {
    Fill {
        paint: rgb::Color::new(r, g, b).into(),
        opacity: NormalizedF32::new(opacidad).unwrap_or(NormalizedF32::ONE),
        rule: FillRule::NonZero,
    }
}

fn texto(s: &mut Surface, letras: &mut Letras, l: &Linea, x: f32, base: f32, em: f32) {
    let Some(fuente) = letras.de(l.lang) else {
        return;
    };
    let glifos: Vec<KrillaGlyph> = l
        .glifos
        .iter()
        .map(|g| {
            KrillaGlyph::new(
                GlyphId::new(g.id),
                g.avance,
                0.0,
                0.0,
                0.0,
                g.rango.clone(),
                None,
            )
        })
        .collect();
    s.draw_glyphs(Point::from_xy(x, base), &glifos, fuente, l.texto, em, false);
}

/// El sello encima de una carta (`trim`, el rectángulo de corte de la casilla).
pub(crate) fn dibujar_sello(
    s: &mut Surface,
    letras: &mut Letras,
    trim: &RectMm,
    sello: &SelloCasilla,
) {
    let l = sello::linea_carta(sello.lang);
    let (ox, oy) = (pt(trim.x), pt(trim.y));
    let c = sello::caja(
        &l,
        f64::from(pt(trim.w)),
        f64::from(pt(trim.h)),
        sello.colocacion.sitio,
    );
    if sello.colocacion.fondo == Fondo::Pastilla {
        let trazos = sello::pastilla(
            ox + c.x0 as f32,
            oy + c.y0 as f32,
            ox + c.x1 as f32,
            oy + c.y1 as f32,
        );
        if let Some(p) = camino(&trazos) {
            s.set_fill(Some(relleno(0, 0, 0, sello::OPACIDAD_PASTILLA)));
            s.draw_path(&p);
        }
    }
    let (r, g, b) = sello.colocacion.color();
    s.set_fill(Some(relleno(r, g, b, 1.0)));
    texto(
        s,
        letras,
        &l,
        ox + c.x as f32,
        oy + c.base as f32,
        c.em as f32,
    );
    s.set_fill(Some(Fill::default()));
}

/// Dónde va la línea del margen de una hoja: centrada en el margen de abajo, por debajo de las
/// cartas y de las guías; si no cabe, en el de arriba; si tampoco, no va. Devuelve la línea
/// base (mm desde arriba).
pub(crate) fn base_hoja(p: &Page) -> Option<f64> {
    let em_mm = f64::from(TAM_HOJA_PT) * 25.4 / 72.0;
    let mut abajo = 0.0f64;
    let mut arriba = p.height_mm;
    for slot in &p.slots {
        abajo = abajo.max(slot.bleed.y + slot.bleed.h);
        arriba = arriba.min(slot.bleed.y);
    }
    for g in &p.guides {
        abajo = abajo.max(g.y1.max(g.y2));
        arriba = arriba.min(g.y1.min(g.y2));
    }
    let hueco_abajo = p.height_mm - abajo;
    if hueco_abajo >= HUECO_HOJA_MM {
        return Some(abajo + hueco_abajo / 2.0 + em_mm * 0.35);
    }
    if arriba >= HUECO_HOJA_MM {
        return Some(arriba / 2.0 + em_mm * 0.35);
    }
    None
}

/// La línea del margen («Hecho con Tapete · No a la venta», en el idioma de la app).
pub(crate) fn dibujar_linea_hoja(s: &mut Surface, letras: &mut Letras, p: &Page, lang: DesignLang) {
    let Some(base) = base_hoja(p) else {
        return;
    };
    let l = sello::linea(sello::texto_hoja(lang), lang);
    let ancho = l.ancho * TAM_HOJA_PT;
    let x = (pt(p.width_mm) - ancho) / 2.0;
    s.set_fill(Some(relleno(GRIS_HOJA, GRIS_HOJA, GRIS_HOJA, 1.0)));
    texto(s, letras, &l, x, pt(base), TAM_HOJA_PT);
    s.set_fill(Some(Fill::default()));
}
