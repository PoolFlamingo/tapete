//! **Amigos y mensajes** con el servidor de Tapete (plan Tapete, Fase D): solicitudes de
//! amistad, mensajes cifrados de extremo a extremo e invitaciones a partidas. La partida sigue
//! siendo P2P con iroh: el servidor solo guarda quién es amigo de quién y bytes que no puede leer.
//!
//! - `firma.rs`: la firma de cada petición (`X-Tapete-*`), con los vectores del servidor.
//! - `cifrado.rs`: la clave de cifrado derivada de la identidad, cada mensaje y el número de
//!   seguridad.
//! - `cliente.rs`: las rutas del API.
//! - `sincronizar.rs`: el bucle de `sync` y lo que hace con cada respuesta.
//!
//! 🛑 **Nada sale hacia el servidor sin consentimiento** (D0): hasta que la persona entra en
//! «Amigos» y acepta (`amigos_activar`), la app no lo llama; al salir (`amigos_desactivar`) deja
//! de hacerlo y puede pedir que borre su clave. La identidad es la de la red (iroh): no hay otra
//! clave que guardar.

pub mod avisar;
pub mod cifrado;
pub mod cliente;
pub mod firma;
pub mod sincronizar;

#[cfg(test)]
mod pruebas_servidor;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tokio::sync::Notify;
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// Ajustes guardados (`app_settings`).
pub(crate) const ACTIVO: &str = "amigos.activo";
pub(crate) const CURSOR: &str = "amigos.cursor";
pub(crate) const HOLA: &str = "amigos.hola";
pub(crate) const CODIGO: &str = "amigos.codigo";

/// Un amigo, como lo pinta la pantalla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AmigoVista {
    /// Su identidad (clave ed25519 en hex).
    pub clave: String,
    pub apodo: String,
    /// Visto hace menos de 2,5 minutos.
    pub en_linea: bool,
    /// Cuándo se le vio por última vez (ISO), si se sabe.
    pub visto: Option<String>,
    /// Su clave de cifrado viene firmada por su identidad: se le puede escribir.
    pub clave_valida: bool,
    /// Se comprobó el número de seguridad.
    pub verificado: bool,
    /// Su clave cambió desde la última vez: se avisa hasta que se mira (plan D3).
    pub clave_cambio: bool,
    /// 12 cifras, iguales en los dos equipos («1234 5678 9012»).
    pub numero_seguridad: String,
    pub sin_leer: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SolicitudVista {
    pub id: String,
    pub clave: String,
    pub apodo: String,
    pub creada: Option<String>,
}

/// Un aviso del servidor («hay versión nueva», «mantenimiento el sábado»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AvisoVista {
    pub id: String,
    /// `info` · `warning` · `critical`.
    pub nivel: String,
    /// Por idioma (`es`, `en`): la pantalla elige.
    pub textos: BTreeMap<String, String>,
}

/// Todo lo de amigos que enseña la pantalla. Llega entero con el evento `amigos://estado`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EstadoAmigos {
    /// La persona aceptó usar el servidor (D0).
    pub activo: bool,
    /// Mi código de amigo (`TAP-XXXX-XXXX`), si ya se dio de alta.
    pub codigo: Option<String>,
    /// Mi identidad (para el QR y el número de seguridad).
    pub clave: String,
    pub amigos: Vec<AmigoVista>,
    pub recibidas: Vec<SolicitudVista>,
    pub enviadas: Vec<SolicitudVista>,
    pub bloqueados: Vec<String>,
    pub avisos: Vec<AvisoVista>,
    /// El último fallo al hablar con el servidor (se limpia con la siguiente que vaya bien).
    pub error: Option<AppError>,
    /// Cuándo fue la última sincronización buena (ms).
    #[ts(type = "number | null")]
    pub sincronizado: Option<i64>,
}

/// El estado de amigos de la app (en `AppState`).
#[derive(Default)]
pub struct Amigos {
    cliente: OnceLock<cliente::Cliente>,
    pub(crate) estado: Mutex<EstadoAmigos>,
    /// Lo último que dijo el servidor de cada amigo (para escribirles y descifrar).
    pub(crate) amigos: Mutex<BTreeMap<String, cliente::AmigoServidor>>,
    pub(crate) chat_abierto: AtomicBool,
    pub(crate) en_segundo_plano: AtomicBool,
    pub(crate) en_marcha: AtomicBool,
    pub(crate) despertar: Notify,
    /// Una sincronización a la vez (el servidor admite 3 peticiones a la vez por clave).
    pub(crate) turno: tokio::sync::Mutex<()>,
}

impl Amigos {
    pub fn cliente(&self) -> AppResult<&cliente::Cliente> {
        if let Some(c) = self.cliente.get() {
            return Ok(c);
        }
        let c = cliente::Cliente::nuevo(cliente::base())?;
        Ok(self.cliente.get_or_init(|| c))
    }

    pub fn estado(&self) -> EstadoAmigos {
        self.estado.lock().map(|e| e.clone()).unwrap_or_default()
    }

    /// Pide una sincronización ya (al abrir un chat, al volver, tras mandar algo).
    pub fn ya(&self) {
        self.despertar.notify_one();
    }

    pub fn ritmo(&self, chat_abierto: bool) {
        let antes = self.chat_abierto.swap(chat_abierto, Ordering::SeqCst);
        if chat_abierto && !antes {
            self.ya();
        }
    }

    pub fn segundo_plano(&self, si: bool) {
        self.en_segundo_plano.store(si, Ordering::SeqCst);
        if !si {
            self.ya();
        }
    }
}

pub(crate) fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
