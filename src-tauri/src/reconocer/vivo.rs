//! **El escáner en vivo** (Android): reconocer cartas sin pulsar nada, como
//! ManaBox, sin calentar el móvil y sin contar dos veces la misma carta.
//!
//! Kotlin (`EscanerCartas.kt`) pone la cámara detrás de la vista web y manda
//! fotogramas por JNI (`Nativo.fotograma`), pero solo si Rust los quiere
//! (`Nativo.quiere`): hay **un hueco de un fotograma** y un hilo que lo vacía.
//! Por cada uno se busca la carta (`foto::detectar`, unos milisegundos) y se
//! le manda a la pantalla dónde está; cuando la carta se queda quieta
//! (`Seguimiento`) se endereza y se lee entera (OCR, nombre, impresión). Lo
//! reconocido va por el canal de la pantalla. La misma carta no se vuelve a
//! mandar hasta que sale del encuadre.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use image::RgbaImage;
use proxy_core::foto::{self, Esquinas, Punto};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::Reconocida;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::state::AppState;

/// Fotogramas seguidos con la carta quieta antes de leerla.
const ESTABLE: u32 = 2;
/// Cuánto se puede mover la carta (en proporción a la diagonal del
/// fotograma) y seguir «quieta».
const QUIETA: f32 = 0.02;
/// Un salto mayor que este es otra carta (o se movió mucho): se puede leer
/// otra vez.
const SALTO: f32 = 0.15;
/// Fotogramas seguidos sin carta para darla por ida (y poder contarla otra vez).
const SALIDA: u32 = 6;
/// Fotogramas que se espera tras una lectura sin resultado.
const REINTENTO: u32 = 3;
/// Por debajo de esta nitidez (`foto::nitidez`) la carta está movida o
/// desenfocada y no se lee.
const NITIDEZ_MINIMA: f32 = 20.0;

/// Lo que el escáner le cuenta a la pantalla.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "tipo", rename_all = "camelCase")]
#[ts(export)]
pub enum EstadoEscaner {
    /// Un fotograma: dónde está la carta (en fracciones del fotograma, ya
    /// derecho), su medida (para encajarlo en la vista previa) y si se está
    /// leyendo.
    Viendo {
        esquinas: Option<[Punto; 4]>,
        ancho: u32,
        alto: u32,
        leyendo: bool,
    },
    /// Una carta nueva.
    Reconocida { reconocida: Box<Reconocida> },
    /// No se pudo leer (sin red para Scryfall, por ejemplo); se sigue.
    Fallo { error: AppError },
}

/// Qué hacer con un fotograma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    Nada,
    Leer,
}

/// Cuándo leer y cuándo una carta es nueva. Sin cámara ni OCR: se prueba con
/// secuencias de esquinas.
#[derive(Debug, Default)]
pub struct Seguimiento {
    anteriores: Option<Esquinas>,
    estable: u32,
    vacios: u32,
    /// La última carta reconocida mientras no salga del encuadre.
    a_la_vista: Option<String>,
    /// La carta que hay ahora ya se leyó: no se relee hasta que salga o cambie.
    leida: bool,
    espera: u32,
}

/// Lo que se ha movido la carta: la media de lo que se movió cada esquina.
fn desplazamiento(a: &Esquinas, b: &Esquinas) -> f32 {
    a.iter().zip(b).map(|(p, q)| p.distancia(*q)).sum::<f32>() / 4.0
}

impl Seguimiento {
    /// Un fotograma nuevo con la carta en `esquinas` (o sin carta).
    /// `diagonal`: la del fotograma, en las mismas unidades.
    pub fn paso(&mut self, esquinas: Option<&Esquinas>, diagonal: f32) -> Paso {
        let Some(e) = esquinas else {
            self.vacios += 1;
            self.estable = 0;
            self.anteriores = None;
            if self.vacios >= SALIDA {
                self.a_la_vista = None;
                self.leida = false;
            }
            return Paso::Nada;
        };
        self.vacios = 0;
        let movido = self
            .anteriores
            .map(|a| desplazamiento(&a, e) / diagonal.max(1.0));
        match movido {
            Some(m) if m < QUIETA => self.estable += 1,
            Some(m) if m > SALTO => {
                self.estable = 1;
                self.leida = false;
            }
            _ => self.estable = 1,
        }
        self.anteriores = Some(*e);
        if self.espera > 0 {
            self.espera -= 1;
            return Paso::Nada;
        }
        if self.leida || self.estable < ESTABLE {
            return Paso::Nada;
        }
        Paso::Leer
    }

    /// El resultado de leer: `true` si es una carta nueva que hay que mandar.
    pub fn leida(&mut self, mejor: Option<String>) -> bool {
        let Some(id) = mejor else {
            self.espera = REINTENTO;
            return false;
        };
        self.leida = true;
        if self.a_la_vista.as_deref() == Some(id.as_str()) {
            return false;
        }
        self.a_la_vista = Some(id);
        true
    }

    /// La carta estaba movida: se espera un fotograma.
    pub fn borrosa(&mut self) {
        self.espera = 1;
    }
}

/// Un fotograma de la cámara: RGBA seguido, sin relleno entre filas, y lo
/// que hay que girarlo (en el sentido del reloj) para que quede derecho.
pub struct Fotograma {
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
    pub giro: u32,
}

/// Lo que comparten el hilo de trabajo y la entrada de JNI.
struct Compartido {
    hueco: Mutex<Option<Fotograma>>,
    hay: Condvar,
    parar: AtomicBool,
    ocupado: AtomicBool,
}

static ACTIVO: Mutex<Option<Arc<Compartido>>> = Mutex::new(None);

fn activo() -> Option<Arc<Compartido>> {
    match ACTIVO.lock() {
        Ok(g) => g.clone(),
        Err(e) => e.into_inner().clone(),
    }
}

/// ¿Quiere Rust un fotograma ahora? (Así Kotlin no copia los que se tirarían.)
pub fn quiere() -> bool {
    activo().is_some_and(|c| {
        !c.ocupado.load(Ordering::SeqCst) && c.hueco.lock().map(|h| h.is_none()).unwrap_or(false)
    })
}

/// Deja un fotograma en el hueco (pisa al que no se haya cogido). `false` si
/// el escáner no está abierto.
pub fn entregar(f: Fotograma) -> bool {
    let Some(c) = activo() else {
        return false;
    };
    match c.hueco.lock() {
        Ok(mut h) => *h = Some(f),
        Err(_) => return false,
    }
    c.hay.notify_one();
    true
}

/// El fotograma como imagen, ya derecho.
fn a_imagen(f: Fotograma) -> Option<RgbaImage> {
    let img = RgbaImage::from_raw(f.ancho, f.alto, f.rgba)?;
    Some(match f.giro % 360 {
        90 => image::imageops::rotate90(&img),
        180 => image::imageops::rotate180(&img),
        270 => image::imageops::rotate270(&img),
        _ => img,
    })
}

/// Espera el siguiente fotograma (o `None` si hay que parar).
fn siguiente(c: &Compartido) -> Option<Fotograma> {
    let mut h = c.hueco.lock().ok()?;
    loop {
        if c.parar.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(f) = h.take() {
            return Some(f);
        }
        h = c.hay.wait_timeout(h, Duration::from_millis(500)).ok()?.0;
    }
}

fn trabajar(app: AppHandle, c: Arc<Compartido>, canal: Channel<EstadoEscaner>) {
    let mut seguimiento = Seguimiento::default();
    while let Some(f) = siguiente(&c) {
        c.ocupado.store(true, Ordering::SeqCst);
        if let Some(img) = a_imagen(f) {
            procesar(&app, &canal, &mut seguimiento, &img);
        }
        c.ocupado.store(false, Ordering::SeqCst);
    }
}

fn procesar(
    app: &AppHandle,
    canal: &Channel<EstadoEscaner>,
    seg: &mut Seguimiento,
    img: &RgbaImage,
) {
    let (w, h) = (img.width(), img.height());
    let esquinas = foto::detectar(img);
    let diagonal = ((w * w + h * h) as f32).sqrt();
    let paso = seg.paso(esquinas.as_ref(), diagonal);
    let fracciones = esquinas.map(|e| {
        e.map(|p| Punto {
            x: p.x / w as f32,
            y: p.y / h as f32,
        })
    });
    let _ = canal.send(EstadoEscaner::Viendo {
        esquinas: fracciones,
        ancho: w,
        alto: h,
        leyendo: paso == Paso::Leer,
    });
    let (Paso::Leer, Some(e)) = (paso, esquinas) else {
        return;
    };
    let Some(carta) = foto::enderezar(img, &e) else {
        return;
    };
    if foto::nitidez(&carta) < NITIDEZ_MINIMA {
        seg.borrosa();
        return;
    }
    let state = app.state::<AppState>();
    match tauri::async_runtime::block_on(super::reconocer_enderezada(&state, fracciones, carta)) {
        Ok(r) => {
            if seg.leida(r.mejor.clone()) {
                let _ = canal.send(EstadoEscaner::Reconocida {
                    reconocida: Box::new(r),
                });
            }
        }
        Err(error) => {
            seg.leida(None);
            let _ = canal.send(EstadoEscaner::Fallo { error });
        }
    }
}

/// Empieza a atender fotogramas (antes de abrir la cámara). Si ya había un
/// escáner abierto, lo para.
pub fn empezar(app: &AppHandle, canal: Channel<EstadoEscaner>) -> AppResult<()> {
    parar();
    let c = Arc::new(Compartido {
        hueco: Mutex::new(None),
        hay: Condvar::new(),
        parar: AtomicBool::new(false),
        ocupado: AtomicBool::new(false),
    });
    match ACTIVO.lock() {
        Ok(mut g) => *g = Some(c.clone()),
        Err(_) => return Err(AppError::new(ErrorCode::Internal).with("detalle", "escáner")),
    }
    let app = app.clone();
    std::thread::Builder::new()
        .name("escaner-cartas".into())
        .spawn(move || trabajar(app, c, canal))
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e.to_string()))?;
    Ok(())
}

/// Deja de atender fotogramas (el hilo acaba al terminar el que tenga entre manos).
pub fn parar() {
    let viejo = match ACTIVO.lock() {
        Ok(mut g) => g.take(),
        Err(e) => e.into_inner().take(),
    };
    if let Some(c) = viejo {
        c.parar.store(true, Ordering::SeqCst);
        c.hay.notify_all();
    }
}

/// `Nativo.quiere()`: ¿mando un fotograma?
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_estigio_proxybuilder_extra_Nativo_quiere<'l>(
    _env: jni::JNIEnv<'l>,
    _clase: jni::objects::JClass<'l>,
) -> jni::sys::jboolean {
    let r = std::panic::catch_unwind(quiere).unwrap_or(false);
    u8::from(r)
}

/// `Nativo.fotograma(rgba, ancho, alto, giro)`: un fotograma de la cámara.
/// `true` si se cogió.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_estigio_proxybuilder_extra_Nativo_fotograma<'l>(
    env: jni::JNIEnv<'l>,
    _clase: jni::objects::JClass<'l>,
    rgba: jni::objects::JByteArray<'l>,
    ancho: jni::sys::jint,
    alto: jni::sys::jint,
    giro: jni::sys::jint,
) -> jni::sys::jboolean {
    let Ok(bytes) = env.convert_byte_array(&rgba) else {
        return 0;
    };
    let (Ok(ancho), Ok(alto), Ok(giro)) = (
        u32::try_from(ancho),
        u32::try_from(alto),
        u32::try_from(giro),
    ) else {
        return 0;
    };
    if bytes.len() != (ancho as usize) * (alto as usize) * 4 {
        return 0;
    }
    let r = std::panic::catch_unwind(move || {
        entregar(Fotograma {
            rgba: bytes,
            ancho,
            alto,
            giro,
        })
    });
    u8::from(r.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIAGONAL: f32 = 1000.0;

    fn carta(x: f32) -> Esquinas {
        [(0.0, 0.0), (300.0, 0.0), (300.0, 420.0), (0.0, 420.0)]
            .map(|(a, b)| Punto { x: a + x, y: b })
    }

    /// Lo que pasa con una secuencia: `Some(x)` = la carta desplazada `x` px.
    fn pasos(s: &mut Seguimiento, secuencia: &[Option<f32>]) -> Vec<Paso> {
        secuencia
            .iter()
            .map(|x| s.paso(x.map(carta).as_ref(), DIAGONAL))
            .collect()
    }

    #[test]
    fn se_lee_cuando_la_carta_se_queda_quieta() {
        let mut s = Seguimiento::default();
        // Entra moviéndose, luego se queda quieta.
        let v = pasos(&mut s, &[Some(0.0), Some(60.0), Some(61.0), Some(62.0)]);
        assert_eq!(v, vec![Paso::Nada, Paso::Nada, Paso::Leer, Paso::Leer]);
    }

    #[test]
    fn leida_no_se_relee_ni_se_repite_mientras_sigue_a_la_vista() {
        let mut s = Seguimiento::default();
        pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert!(s.leida(Some("bolt".into())));
        let v = pasos(&mut s, &[Some(0.0); 20]);
        assert!(v.iter().all(|p| *p == Paso::Nada));
        // Un hueco corto (un parpadeo de la detección) no la da por ida.
        pasos(&mut s, &[None, None, Some(0.0), Some(0.0)]);
        assert_eq!(s.a_la_vista.as_deref(), Some("bolt"));
    }

    #[test]
    fn la_misma_carta_cuenta_otra_vez_si_sale_y_vuelve() {
        let mut s = Seguimiento::default();
        pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert!(s.leida(Some("bolt".into())));
        pasos(&mut s, &[None; SALIDA as usize]);
        let v = pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert_eq!(v[1], Paso::Leer);
        assert!(s.leida(Some("bolt".into())));
    }

    #[test]
    fn otra_carta_en_su_sitio_se_lee_y_cuenta() {
        let mut s = Seguimiento::default();
        pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert!(s.leida(Some("bolt".into())));
        // Un salto grande: otra carta (sin salir del todo del encuadre).
        let v = pasos(&mut s, &[Some(400.0), Some(400.0)]);
        assert_eq!(v[1], Paso::Leer);
        assert!(s.leida(Some("shock".into())));
        // Y si la de antes vuelve igual de rápido, también cuenta.
        pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert!(s.leida(Some("bolt".into())));
    }

    #[test]
    fn el_mismo_nombre_tras_un_salto_no_se_repite() {
        let mut s = Seguimiento::default();
        pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert!(s.leida(Some("bolt".into())));
        pasos(&mut s, &[Some(400.0), Some(400.0)]);
        assert!(!s.leida(Some("bolt".into())));
    }

    #[test]
    fn sin_resultado_espera_unos_fotogramas_antes_de_reintentar() {
        let mut s = Seguimiento::default();
        pasos(&mut s, &[Some(0.0), Some(0.0)]);
        assert!(!s.leida(None));
        let v = pasos(&mut s, &[Some(0.0); (REINTENTO + 1) as usize]);
        assert_eq!(v.iter().filter(|p| **p == Paso::Leer).count(), 1);
        assert_eq!(v.last(), Some(&Paso::Leer));
    }

    #[test]
    fn sin_escaner_abierto_no_se_quieren_fotogramas() {
        parar();
        assert!(!quiere());
        assert!(!entregar(Fotograma {
            rgba: vec![0; 4],
            ancho: 1,
            alto: 1,
            giro: 0,
        }));
    }

    #[test]
    fn el_giro_deja_el_fotograma_derecho() {
        let f = Fotograma {
            rgba: vec![0; 4 * 6],
            ancho: 3,
            alto: 2,
            giro: 90,
        };
        let img = a_imagen(f).expect("imagen");
        assert_eq!((img.width(), img.height()), (2, 3));
    }
}
