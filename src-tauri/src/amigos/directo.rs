//! **Avisos en directo** del servidor (`GET /events`, SSE): mientras la app está delante, una
//! conexión abierta por la que llega `cambio` (una solicitud, una aceptación, un mensaje, un acuse,
//! un amigo que se conecta…) y `escribiendo`. Con `cambio` (y `hola`, al conectar) se despierta el
//! bucle de `sincronizar.rs`, que hace el `sync` de siempre: aquí no se lee ni se guarda nada más.
//!
//! El `sync` cada 60 s sigue debajo como red de seguridad; mientras esto está conectado, abrir un
//! chat ya no lo acelera a 10 s. Si se corta, se reconecta con espera creciente (2 s… 2 min). Con
//! un servidor sin `/events` (404), se deja y queda solo el `sync` periódico. En segundo plano (el
//! móvil) se cierra, y al volver se abre otra vez (`Amigos::segundo_plano`).

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use super::cliente::{EventoDirecto, LectorSse};
use super::sincronizar::activo;
use crate::error::{AppResult, ErrorCode};
use crate::state::AppState;

const ESPERA_MIN: Duration = Duration::from_secs(2);
const ESPERA_MAX: Duration = Duration::from_secs(120);

/// Arranca la escucha si la persona aceptó usar el servidor y no está ya en marcha.
pub fn arrancar(app: AppHandle) {
    let st = app.state::<AppState>();
    if !activo(&st) || st.amigos.escuchando.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut espera = ESPERA_MIN;
        loop {
            let st = app.state::<AppState>();
            if !activo(&st) {
                break;
            }
            if st.amigos.en_segundo_plano.load(Ordering::SeqCst) {
                // Hasta que vuelva: `segundo_plano(false)` avisa por `cortar` y, por si el aviso
                // llega justo antes de esperarlo, se mira otra vez cada pocos segundos.
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    _ = st.amigos.cortar.notified() => {}
                }
                continue;
            }
            let conectado_antes = std::time::Instant::now();
            match escuchar(&app).await {
                Ok(()) => {}
                Err(e) if e.code == ErrorCode::NotFound => {
                    tracing::info!("amigos: el servidor no tiene /events; solo el sync periódico");
                    break;
                }
                Err(e) => tracing::info!(codigo = ?e.code, "amigos: /events se cortó"),
            }
            st.amigos.en_directo.store(false, Ordering::SeqCst);
            // Una conexión que aguantó un rato no cuenta como fallo.
            if conectado_antes.elapsed() > Duration::from_secs(60) {
                espera = ESPERA_MIN;
            }
            tokio::select! {
                _ = tokio::time::sleep(espera) => {}
                _ = st.amigos.cortar.notified() => {}
            }
            espera = (espera * 2).min(ESPERA_MAX);
        }
        let st = app.state::<AppState>();
        st.amigos.en_directo.store(false, Ordering::SeqCst);
        st.amigos.escuchando.store(false, Ordering::SeqCst);
    });
}

/// Una conexión, hasta que se corte o se pida cortarla (segundo plano, salir, otra identidad).
async fn escuchar(app: &AppHandle) -> AppResult<()> {
    let st = app.state::<AppState>();
    let clave = st.red.clave();
    let mut r = st.amigos.cliente()?.abrir_directo(&clave).await?;
    // Mientras esté abierta, sigue valiendo si la app sigue delante, con amigos y con esta clave
    // (se mira con cada trozo; el latido llega cada 20 s).
    let sigue = || {
        activo(&st)
            && !st.amigos.en_segundo_plano.load(Ordering::SeqCst)
            && st.red.clave().public() == clave.public()
    };
    st.amigos.en_directo.store(true, Ordering::SeqCst);
    let mut lector = LectorSse::default();
    loop {
        let trozo = tokio::select! {
            t = r.chunk() => t.map_err(crate::error::red)?,
            _ = st.amigos.cortar.notified() => return Ok(()),
        };
        let Some(trozo) = trozo else {
            return Ok(());
        };
        if !sigue() {
            return Ok(());
        }
        for e in lector.leer(&trozo) {
            match e {
                EventoDirecto::Hola | EventoDirecto::Cambio => st.amigos.ya(),
                EventoDirecto::Escribiendo(de) => {
                    let _ = app.emit("amigos://escribiendo", de);
                }
            }
        }
    }
}
