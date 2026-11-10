//! **Avisos del sistema** cuando llega un mensaje (plan D4b): solo si la ventana no tiene el
//! foco, y nunca con el texto del mensaje (solo «Mensaje de <apodo>»).
//!
//! Rust no escribe prosa: la pantalla le pasa los textos ya traducidos con `t()`
//! (`amigos_textos`, al arrancar y al cambiar de idioma) y aquí solo se pone el apodo.
//! En el escritorio, la notificación del sistema; en Android, la del plugin (con la app abierta;
//! con la app cerrada, el `sync` periódico de WorkManager, aparte: `fondo.rs`).

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use ts_rs::TS;

use crate::state::AppState;
use crate::store::amigos as almacen;

/// Los textos de los avisos, ya traducidos. `{apodo}` se cambia por el del amigo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TextosAvisos {
    pub titulo: String,
    pub mensaje_de: String,
    pub invitacion_de: String,
}

static TEXTOS: Mutex<Option<TextosAvisos>> = Mutex::new(None);

pub fn poner_textos(t: TextosAvisos) {
    if let Ok(mut g) = TEXTOS.lock() {
        *g = Some(t);
    }
}

fn ventana_con_foco(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false)
}

/// El cuerpo del aviso: el texto que toca con el apodo puesto (nunca el mensaje).
pub(crate) fn cuerpo(t: &TextosAvisos, apodo: &str, invitacion: bool) -> String {
    let plantilla = if invitacion {
        &t.invitacion_de
    } else {
        &t.mensaje_de
    };
    plantilla.replace("{apodo}", apodo)
}

pub fn mensajes_nuevos(app: &AppHandle, amigos: &[String]) {
    if ventana_con_foco(app) {
        return;
    }
    let Some(textos) = TEXTOS.lock().ok().and_then(|g| g.clone()) else {
        return;
    };
    let st = app.state::<AppState>();
    let conocidos = st
        .amigos
        .amigos
        .lock()
        .map(|a| a.clone())
        .unwrap_or_default();
    let alias = almacen::alias(&st.db).unwrap_or_default();
    for clave in amigos {
        // El nombre que le puse yo, si le puse uno; si no, el suyo.
        let apodo = alias.get(clave).cloned().unwrap_or_else(|| {
            conocidos
                .get(clave)
                .map_or_else(|| clave.chars().take(8).collect(), |a| a.nickname.clone())
        });
        let invitacion = almacen::conversacion(&st.db, clave, None, 1)
            .ok()
            .and_then(|v| v.last().map(|m| m.tipo == "invitacion"))
            .unwrap_or(false);
        let texto = cuerpo(&textos, &apodo, invitacion);
        // En Android, el aviso propio (el del plugin de Tauri sale con un icono «ⓘ» y, expandido,
        // sin el texto).
        if cfg!(target_os = "android") {
            if let Err(e) = crate::android::aviso_amigo(app, clave, &textos.titulo, &texto) {
                tracing::info!(codigo = ?e.code, "amigos: no se pudo avisar");
            }
            continue;
        }
        let r = app
            .notification()
            .builder()
            .title(&textos.titulo)
            .body(texto)
            .show();
        if let Err(e) = r {
            tracing::info!("amigos: no se pudo avisar: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_aviso_lleva_el_apodo_y_nunca_el_texto() {
        let t = TextosAvisos {
            titulo: "Tapete".into(),
            mensaje_de: "Mensaje de {apodo}".into(),
            invitacion_de: "{apodo} te invita a una partida".into(),
        };
        assert_eq!(cuerpo(&t, "Ana", false), "Mensaje de Ana");
        assert_eq!(cuerpo(&t, "Ana", true), "Ana te invita a una partida");
    }
}
