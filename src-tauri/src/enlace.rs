//! **Enlaces `tapete://` y `proxybuilder://`**: un enlace de invitación abre la app con el
//! código ya puesto. Desde la retirada de las salas del servidor (plan Tapete, D7):
//!
//! - `tapete://unirse/<código directo>` (`PB1-…`): «Jugar en red» con el código puesto.
//! - `tapete://amigo/<código de amigo>` (`TAP-XXXX-XXXX`): «Amigos» con el código puesto.
//!
//! `proxybuilder://` se sigue aceptando con el mismo formato (los enlaces ya enviados y las
//! versiones anteriores a la 4.0, que lo generaban). El enlace **no hace nada por sí solo**:
//! Rust solo comprueba el código y se lo pasa a la pantalla; unirse o pedir amistad lo pulsa la
//! persona. Cualquier otra cosa (otro esquema, otra ruta, un código mal formado, un enlace
//! enorme) se ignora.
//!
//! Los enlaces que la app genera para compartir son los de la web
//! (`https://tapete.zodhub.app/join/<código>` y `/friend/<código>`), que abren la app con el
//! esquema que entienda la versión de cada uno o llevan a descargarla.
//!
//! Se escucha aquí, en Rust (`tauri-plugin-deep-link`, y `tauri-plugin-single-instance` para
//! que una segunda instancia lance el enlace a la primera en Windows y Linux), y a la pantalla
//! le llega el evento `enlace://entrante`. Si el enlace es el que abrió la app, la pantalla aún
//! no escucha: queda guardado y lo pide con `enlace_pendiente`.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::state::AppState;

/// El esquema del nombre nuevo (el que abre la web).
pub const ESQUEMA: &str = "tapete";
/// Los esquemas que se aceptan, los dos registrados en `tauri.conf.json`
/// (`plugins.deep-link`, escritorio y móvil) y en el `intent-filter` de Android.
pub const ESQUEMAS: [&str; 2] = ["proxybuilder", ESQUEMA];
/// La web que abre la app (y lleva a descargarla si no está).
pub const WEB: &str = "https://tapete.zodhub.app";
/// Un enlace más largo que esto no es de la app (un código directo cabe en 4 KiB).
const MAX_ENLACE: usize = 4_200;
const MAX_CODIGO_PARTIDA: usize = 4_096;

/// Lo que trae un enlace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "tipo", content = "codigo", rename_all = "camelCase")]
#[ts(export)]
pub enum EnlaceEntrante {
    /// Un código directo de partida (`PB1-…`).
    Unirse(String),
    /// Un código de amigo, ya normalizado (`TAP-XXXX-XXXX`).
    Amigo(String),
}

const CROCKFORD: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// El código de amigo como lo guarda el servidor: 8 caracteres de Crockford (con o sin `TAP-`,
/// guiones, espacios; I y L se leen como 1 y O como 0) → `TAP-XXXX-XXXX`.
pub fn codigo_amigo(texto: &str) -> Option<String> {
    let t = texto.trim();
    let t = t
        .strip_prefix("TAP-")
        .or_else(|| t.strip_prefix("tap-"))
        .unwrap_or(t);
    let c: String = t
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            x => x,
        })
        .collect();
    (c.chars().count() == 8 && c.chars().all(|x| CROCKFORD.contains(x)))
        .then(|| format!("TAP-{}-{}", &c[..4], &c[4..]))
}

/// El código de amigo de lo que se pega o se lee de un QR: el código suelto, el enlace de la web
/// (`…/friend/<código>`) o el de la app (`tapete://amigo/<código>`).
pub fn codigo_amigo_pegado(texto: &str) -> Option<String> {
    let t = texto.trim();
    if t.len() > MAX_ENLACE {
        return None;
    }
    codigo_amigo(t).or_else(|| {
        let (_, resto) = t
            .rsplit_once("/friend/")
            .or_else(|| t.rsplit_once("://amigo/"))?;
        codigo_amigo(resto.split(['/', '?', '#']).next()?)
    })
}

/// Un código directo de partida con pinta de serlo (Rust lo comprueba de verdad al unirse).
fn codigo_partida(texto: &str) -> Option<String> {
    let resto = texto.strip_prefix("PB1-")?;
    (resto.len() >= 8
        && texto.len() <= MAX_CODIGO_PARTIDA
        && resto
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then(|| texto.to_string())
}

/// Lo que trae un enlace `tapete://…` o `proxybuilder://…`, o `None` si no es de la app.
pub fn leer(enlace: &str) -> Option<EnlaceEntrante> {
    if enlace.len() > MAX_ENLACE {
        return None;
    }
    let (esquema, resto) = enlace.split_once("://")?;
    if !ESQUEMAS.iter().any(|e| esquema.eq_ignore_ascii_case(e)) {
        return None;
    }
    // Sin la consulta ni el fragmento.
    let resto = resto.split(['?', '#']).next()?;
    let mut partes = resto.trim_matches('/').split('/');
    let ruta = partes.next()?;
    let codigo = partes.next()?;
    // Ni nada más detrás.
    if partes.next().is_some() {
        return None;
    }
    if ruta.eq_ignore_ascii_case("unirse") {
        codigo_partida(codigo).map(EnlaceEntrante::Unirse)
    } else if ruta.eq_ignore_ascii_case("amigo") {
        codigo_amigo(codigo).map(EnlaceEntrante::Amigo)
    } else {
        None
    }
}

/// El enlace que lo pendiente (por si la pantalla aún no escucha).
static PENDIENTE: Mutex<Option<EnlaceEntrante>> = Mutex::new(None);

fn pendiente() -> std::sync::MutexGuard<'static, Option<EnlaceEntrante>> {
    match PENDIENTE.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
}

/// Llegan enlaces (de la primera instancia o de una segunda): el primero que vale se guarda y
/// se avisa a la pantalla, y la ventana pasa al frente.
pub fn recibir<S: AsRef<str>>(app: &AppHandle, enlaces: impl IntoIterator<Item = S>) {
    let Some(e) = enlaces.into_iter().find_map(|e| leer(e.as_ref())) else {
        return;
    };
    *pendiente() = Some(e.clone());
    let _ = app.emit("enlace://entrante", &e);
    if let Some(v) = app.get_webview_window("main") {
        let _ = v.show();
        #[cfg(desktop)]
        let _ = v.unminimize();
        let _ = v.set_focus();
    }
}

/// El enlace pendiente, una sola vez: la pantalla lo pide al arrancar.
#[tauri::command]
pub fn enlace_pendiente(_state: tauri::State<'_, AppState>) -> Option<EnlaceEntrante> {
    pendiente().take()
}

/// El enlace de la web para invitar a una partida con un código directo: sin las IP del
/// anfitrión (solo el relé), como en las invitaciones a amigos.
#[tauri::command]
pub fn enlace_invitacion(codigo: String) -> crate::error::AppResult<String> {
    let solo = crate::red::codigo::solo_rele(codigo.trim())?;
    Ok(format!("{WEB}/join/{solo}"))
}

/// El enlace de la web para que alguien te añada de amigo.
#[tauri::command]
pub fn enlace_amigo(codigo: String) -> Option<String> {
    codigo_amigo(&codigo).map(|c| format!("{WEB}/friend/{c}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partida() -> String {
        format!("PB1-{}", "AbC_d-9".repeat(10))
    }

    #[test]
    fn unirse_con_cualquiera_de_los_dos_esquemas() {
        let c = partida();
        for e in [
            format!("tapete://unirse/{c}"),
            format!("Tapete://UNIRSE/{c}/"),
            format!("proxybuilder://unirse/{c}?utm=x#y"),
        ] {
            assert_eq!(leer(&e), Some(EnlaceEntrante::Unirse(c.clone())), "{e}");
        }
    }

    #[test]
    fn un_codigo_de_amigo_sale_normalizado() {
        for e in [
            "tapete://amigo/TAP-7K2M-9QXD",
            "tapete://amigo/7k2m9qxd",
            "proxybuilder://amigo/tap-7K2M 9QXD",
        ] {
            assert_eq!(
                leer(e),
                Some(EnlaceEntrante::Amigo("TAP-7K2M-9QXD".into())),
                "{e}"
            );
        }
        // I, L y O se leen como 1, 1 y 0.
        assert_eq!(codigo_amigo("ILO0ABCD").as_deref(), Some("TAP-1100-ABCD"));
    }

    #[test]
    fn los_esquemas_aceptados_son_los_registrados() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let dl = &conf["plugins"]["deep-link"];
        let lista = |v: &serde_json::Value| -> Vec<String> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect()
        };
        let mut esperados: Vec<String> = ESQUEMAS.iter().map(|s| s.to_string()).collect();
        esperados.sort();
        let mut escritorio = lista(&dl["desktop"]["schemes"]);
        escritorio.sort();
        let mut movil = lista(&dl["mobile"][0]["scheme"]);
        movil.sort();
        assert_eq!(escritorio, esperados);
        assert_eq!(movil, esperados);
        let manifiesto = include_str!("../gen/android/app/src/main/AndroidManifest.xml");
        for e in ESQUEMAS {
            assert!(
                manifiesto.contains(&format!("<data android:scheme=\"{e}\" />")),
                "{e} en el AndroidManifest"
            );
        }
    }

    #[test]
    fn lo_que_no_es_un_enlace_de_la_app_se_ignora() {
        let c = partida();
        for malo in [
            String::new(),
            format!("https://unirse/{c}"),
            format!("tapete://abrir/{c}"),
            "tapete://unirse/".into(),
            "tapete://unirse/ABCDE12345".into(),
            "tapete://unirse/PB1-corto".into(),
            "tapete://unirse/PB1-con espacio".into(),
            format!("tapete://unirse/{c}/mas"),
            "tapete://unirse/../../etc/passwd".into(),
            format!("tapete:unirse/{c}"),
            format!("tapetes://unirse/{c}"),
            "tapete://amigo/TAP-123".into(),
            "tapete://amigo/TAP-UUUU-UUUU".into(),
            format!("tapete://unirse/PB1-{}", "a".repeat(5000)),
        ] {
            assert_eq!(leer(&malo), None, "{malo}");
        }
    }

    #[test]
    fn los_enlaces_para_compartir_son_los_de_la_web() {
        assert_eq!(
            enlace_amigo("7k2m9qxd".into()).as_deref(),
            Some("https://tapete.zodhub.app/friend/TAP-7K2M-9QXD")
        );
        assert_eq!(enlace_amigo("nada".into()), None);
        for pegado in [
            "https://tapete.zodhub.app/friend/TAP-7K2M-9QXD",
            "https://tapete.zodhub.app/friend/7k2m-9qxd/?x=1",
            "tapete://amigo/TAP-7K2M-9QXD",
            " tap-7k2m-9qxd ",
        ] {
            assert_eq!(
                codigo_amigo_pegado(pegado).as_deref(),
                Some("TAP-7K2M-9QXD"),
                "{pegado}"
            );
        }
        assert_eq!(codigo_amigo_pegado("https://otra.web/friend/nada"), None);
        assert!(enlace_invitacion("PB1-xx".into()).is_err());
    }

    #[test]
    fn el_evento_lleva_el_tipo_y_el_codigo() {
        let j = serde_json::to_value(EnlaceEntrante::Amigo("TAP-AAAA-BBBB".into())).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"tipo": "amigo", "codigo": "TAP-AAAA-BBBB"})
        );
    }
}
