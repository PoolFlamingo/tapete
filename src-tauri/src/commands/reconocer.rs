//! **Identificar por foto**: los comandos (la lógica, en `crate::reconocer`;
//! el escáner en vivo, en `crate::reconocer::vivo`).

use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::android::UsoPantalla;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::reconocer::vivo::EstadoEscaner;
use crate::reconocer::{self, Reconocida};
use crate::state::AppState;

/// Lo más que se lee de una foto que llega por una URI de Android (una foto
/// de móvil de 200 MP en PNG anda por los 60 MB).
const TOPE_FOTO: u64 = 80 * 1024 * 1024;

/// Identifica la carta de una foto elegida o soltada (`ruta`: un fichero o,
/// en Android, una URI `content://`).
#[tauri::command]
pub async fn reconocer_foto(
    app: AppHandle,
    state: State<'_, AppState>,
    ruta: String,
) -> AppResult<Reconocida> {
    let bytes = crate::fichero::leer_bytes(&app, &ruta, TOPE_FOTO)?;
    let img =
        tauri::async_runtime::spawn_blocking(move || proxy_core::imaging::decode::decode(&bytes))
            .await
            .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e.to_string()))??;
    reconocer::reconocer(&state, img).await
}

/// Abre el escáner en vivo (solo Android): baja los modelos del OCR si
/// faltan, pone a trabajar `reconocer::vivo` y abre la cámara (que pide el
/// permiso si hace falta). Lo que se ve y se reconoce llega por `canal`.
#[tauri::command]
pub async fn reconocer_vivo_abrir(
    app: AppHandle,
    state: State<'_, AppState>,
    canal: Channel<EstadoEscaner>,
) -> AppResult<()> {
    if !cfg!(target_os = "android") {
        return Err(AppError::new(ErrorCode::Unsupported).with("motivo", "camara"));
    }
    let datos = state.paths.data.clone();
    if !crate::ocr::modelos_listos(&datos) {
        crate::ocr::descargar_modelos(&datos).await?;
    }
    reconocer::vivo::empezar(&app, canal)?;
    let a = app.clone();
    let abierta = tauri::async_runtime::spawn_blocking(move || crate::android::escaner_abrir(&a))
        .await
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e.to_string()))?;
    if let Err(e) = abierta {
        reconocer::vivo::parar();
        return Err(e);
    }
    if let Err(e) = crate::android::pantalla_encendida(&app, UsoPantalla::Escaner, true) {
        tracing::warn!(codigo = ?e.code, "escáner: la pantalla puede apagarse");
    }
    Ok(())
}

/// Cierra el escáner en vivo (también si no estaba abierto).
#[tauri::command]
pub async fn reconocer_vivo_cerrar(app: AppHandle) -> AppResult<()> {
    reconocer::vivo::parar();
    let a = app.clone();
    let cerrada = tauri::async_runtime::spawn_blocking(move || crate::android::escaner_cerrar(&a))
        .await
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e.to_string()))?;
    let _ = crate::android::pantalla_encendida(&app, UsoPantalla::Escaner, false);
    cerrada
}

/// Enciende o apaga la linterna; `false` si no hay.
#[tauri::command]
pub async fn reconocer_vivo_linterna(app: AppHandle, si: bool) -> AppResult<bool> {
    tauri::async_runtime::spawn_blocking(move || crate::android::escaner_linterna(&app, si))
        .await
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e.to_string()))?
}
