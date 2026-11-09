//! **Enlaces `proxybuilder://` y `tapete://`** (Q-F3): un enlace de invitación
//! abre la app y lleva al diálogo de «Con amigos…» con el código puesto.
//!
//! Formato: `proxybuilder://unirse/<código>` o `tapete://unirse/<código>` (el
//! nombre nuevo; entra desde la 3.6.0 para que los enlaces que genere la 4.x
//! abran las copias ya instaladas), con el código de 10 caracteres
//! del servidor (base32 de Crockford). El enlace **no hace nada por sí
//! solo**: Rust solo valida el código y se lo pasa a la pantalla, que lo
//! pone en «Unirse con un código» y espera a que la persona pulse el botón.
//! Cualquier otra cosa (otro esquema, otra ruta, un código mal formado, un
//! enlace enorme) se ignora.
//!
//! Se escucha aquí, en Rust (`tauri-plugin-deep-link`, y
//! `tauri-plugin-single-instance` para que una segunda instancia lance el
//! enlace a la primera en Windows y Linux), y a la pantalla le llega el
//! evento `salas://enlace` con el código. Si el enlace es el que abrió la
//! app, la pantalla aún no escucha: queda guardado y lo pide con
//! `enlace_pendiente`.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// El esquema de los enlaces que genera la app. 🛑 Sigue siendo el viejo hasta
/// que todas las copias entiendan `tapete://` (lo cambia la 4.0.0).
pub const ESQUEMA: &str = "proxybuilder";
/// Los esquemas que se aceptan, los dos registrados en `tauri.conf.json`
/// (`plugins.deep-link`, escritorio y móvil) y en el `intent-filter` de Android.
pub const ESQUEMAS: [&str; 2] = [ESQUEMA, "tapete"];
const ALFABETO: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LARGO: usize = 10;
/// Un enlace más largo que esto no es de la app.
const MAX_ENLACE: usize = 200;

/// El código de invitación como lo guarda el servidor (`normalizar` de
/// `server/src/invitaciones.ts`): sin espacios ni guiones, en mayúsculas y
/// con I, L y O leídas como 1 y 0.
pub fn normalizar(texto: &str) -> Option<String> {
    let c: String = texto
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            x => x,
        })
        .collect();
    (c.chars().count() == LARGO && c.chars().all(|x| ALFABETO.contains(x))).then_some(c)
}

/// El código de un enlace `proxybuilder://unirse/<código>` (o `tapete://`), o `None` si el
/// enlace es de otra cosa o el código no puede ser uno.
pub fn codigo_de(enlace: &str) -> Option<String> {
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
    if !partes.next()?.eq_ignore_ascii_case("unirse") {
        return None;
    }
    let codigo = partes.next()?;
    // Ni nada más detrás.
    if partes.next().is_some() {
        return None;
    }
    normalizar(codigo)
}

/// El enlace de invitación de un código.
pub fn enlace_de(codigo: &str) -> Option<String> {
    normalizar(codigo).map(|c| format!("{ESQUEMA}://unirse/{c}"))
}

/// El enlace que abrió la app (o el último), por si la pantalla aún no escucha.
static PENDIENTE: Mutex<Option<String>> = Mutex::new(None);

fn pendiente() -> std::sync::MutexGuard<'static, Option<String>> {
    match PENDIENTE.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
}

/// Llegan enlaces (de la primera instancia o de una segunda): los que valen
/// se guardan y se avisa a la pantalla, y la ventana pasa al frente.
pub fn recibir<S: AsRef<str>>(app: &AppHandle, enlaces: impl IntoIterator<Item = S>) {
    let Some(codigo) = enlaces.into_iter().find_map(|e| codigo_de(e.as_ref())) else {
        return;
    };
    *pendiente() = Some(codigo.clone());
    let _ = app.emit("salas://enlace", &codigo);
    if let Some(v) = app.get_webview_window("main") {
        let _ = v.show();
        #[cfg(desktop)]
        let _ = v.unminimize();
        let _ = v.set_focus();
    }
}

/// El enlace pendiente, una sola vez: la pantalla lo pide al arrancar.
#[tauri::command]
pub fn enlace_pendiente(_state: tauri::State<'_, AppState>) -> Option<String> {
    pendiente().take()
}

/// El enlace de invitación de un código (el que se copia en el panel).
#[tauri::command]
pub fn enlace_invitacion(codigo: String) -> Option<String> {
    enlace_de(&codigo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_enlace_de_invitacion_da_su_codigo() {
        assert_eq!(
            codigo_de("proxybuilder://unirse/ABCDE12345").as_deref(),
            Some("ABCDE12345")
        );
        // Las barras de más, las mayúsculas y el guion del código bonito.
        assert_eq!(
            codigo_de("ProxyBuilder://UNIRSE/abcde-12345/").as_deref(),
            Some("ABCDE12345")
        );
        // Lo que añaden los navegadores y los chats.
        assert_eq!(
            codigo_de("proxybuilder://unirse/ABCDE12345?utm=x#y").as_deref(),
            Some("ABCDE12345")
        );
        // I, L y O se leen como 1, 1 y 0.
        assert_eq!(
            codigo_de("proxybuilder://unirse/ILOABCDEFG").as_deref(),
            Some("110ABCDEFG")
        );
    }

    #[test]
    fn el_esquema_nuevo_tambien_entra() {
        assert_eq!(
            codigo_de("tapete://unirse/ABCDE12345").as_deref(),
            Some("ABCDE12345")
        );
        assert_eq!(
            codigo_de("Tapete://UNIRSE/abcde-12345/?x=1").as_deref(),
            Some("ABCDE12345")
        );
    }

    /// Los esquemas aceptados son los registrados: si uno se queda sin
    /// registrar, el sistema no abriría la app con él.
    #[test]
    fn los_esquemas_aceptados_son_los_registrados() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let dl = &conf["plugins"]["deep-link"];
        let escritorio: Vec<&str> = dl["desktop"]["schemes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let movil: Vec<&str> = dl["mobile"][0]["scheme"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(escritorio, ESQUEMAS);
        assert_eq!(movil, ESQUEMAS);
        let manifiesto = include_str!("../gen/android/app/src/main/AndroidManifest.xml");
        for e in ESQUEMAS {
            assert!(
                manifiesto.contains(&format!("<data android:scheme=\"{e}\" />")),
                "{e} en el AndroidManifest"
            );
        }
    }

    #[test]
    fn lo_que_no_es_una_invitacion_se_ignora() {
        for malo in [
            "",
            "https://unirse/ABCDE12345",
            "proxybuilder://abrir/ABCDE12345",
            "proxybuilder://unirse/",
            "proxybuilder://unirse/ABCDE1234",
            "proxybuilder://unirse/ABCDE123456",
            "proxybuilder://unirse/ABCDE1234U",
            "proxybuilder://unirse/ABCDE12345/mas",
            "proxybuilder://unirse/../../etc/passwd",
            "proxybuilder:unirse/ABCDE12345",
            "tapete:unirse/ABCDE12345",
            "tapetes://unirse/ABCDE12345",
            "tap://unirse/ABCDE12345",
            "tapete://abrir/ABCDE12345",
            "tapete://unirse/ABCDE1234",
            "http://tapete.zodhub.app/unirse/ABCDE12345",
        ] {
            assert_eq!(codigo_de(malo), None, "{malo}");
        }
        assert_eq!(
            codigo_de(&format!("proxybuilder://unirse/{}", "A".repeat(500))),
            None
        );
    }

    #[test]
    fn el_enlace_de_un_codigo_vuelve_a_dar_el_codigo() {
        let e = enlace_de("abcde-12345").unwrap();
        assert_eq!(e, "proxybuilder://unirse/ABCDE12345");
        assert_eq!(codigo_de(&e).as_deref(), Some("ABCDE12345"));
        assert_eq!(enlace_de("corto"), None);
    }
}
