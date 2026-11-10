//! Comandos de **amigos y mensajes** (plan Tapete, Fase D). La lógica está en `crate::amigos`;
//! aquí solo se valida, se llama y se pide una sincronización para que la pantalla se ponga al
//! día. 🛑 Ninguno habla con el servidor si la persona no aceptó (`amigos_activar`).

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use crate::amigos::avisar::{self, TextosAvisos};
use crate::amigos::cifrado::{self, Carta, Contenido};
use crate::amigos::sincronizar::{self, activo, estado_inicial, publicar};
use crate::amigos::{ahora_ms, EstadoAmigos, ACTIVO, CODIGO, CURSOR, HOLA};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::state::AppState;
use crate::store::amigos::{self as almacen, Mensaje};
use crate::store::settings;

fn apagado() -> AppError {
    AppError::new(ErrorCode::InvalidInput).with("motivo", "amigos_apagado")
}

fn exigir_activo(st: &AppState) -> AppResult<()> {
    if activo(st) {
        Ok(())
    } else {
        Err(apagado())
    }
}

/// Lo último que se sabe (sin llamar al servidor).
#[tauri::command]
pub fn amigos_estado(state: State<'_, AppState>) -> EstadoAmigos {
    let e = state.amigos.estado();
    if e.activo || e.sincronizado.is_some() {
        e
    } else {
        estado_inicial(&state)
    }
}

/// La persona acepta usar el servidor (D0): se presenta, sincroniza y arranca el bucle.
#[tauri::command]
pub async fn amigos_activar(app: AppHandle) -> AppResult<EstadoAmigos> {
    let st = app.state::<AppState>();
    settings::set(&st.db, ACTIVO, "1")?;
    // Con la app cerrada, los avisos los da el sistema (D4b): el permiso se pide ahora, al
    // aceptar usar el servidor, y no antes.
    crate::android::pedir_permiso_avisos(&app);
    sincronizar::arrancar(app.clone());
    if let Err(e) = sincronizar::sincronizar(&app).await {
        let mut actual = estado_inicial(&st);
        actual.error = Some(e.clone());
        publicar(&app, actual);
        return Err(e);
    }
    Ok(st.amigos.estado())
}

/// Deja de usar el servidor. Con `borrar_servidor`, antes le pide que borre esta clave con sus
/// amistades y mensajes; con `borrar_local`, borra también las conversaciones de este equipo.
#[tauri::command]
pub async fn amigos_desactivar(
    app: AppHandle,
    borrar_servidor: bool,
    borrar_local: bool,
) -> AppResult<EstadoAmigos> {
    let st = app.state::<AppState>();
    if borrar_servidor && activo(&st) {
        let clave = st.red.clave();
        match st.amigos.cliente()?.borrarme(&clave).await {
            Ok(()) => {}
            // Ya no estaba: da igual.
            Err(e) if e.params.get("code").map(String::as_str) == Some("not_registered") => {}
            Err(e) => return Err(e),
        }
        for k in [CODIGO, CURSOR, HOLA] {
            settings::set(&st.db, k, "")?;
        }
    }
    settings::set(&st.db, ACTIVO, "0")?;
    // Cierra `/events`; la escucha ve que ya no está activo y se para.
    st.amigos.cortar.notify_waiters();
    if let Err(e) = crate::android::avisos_amigos(&app, None) {
        tracing::info!(codigo = ?e.code, "amigos: no se pudieron cancelar los avisos");
    }
    if borrar_local {
        almacen::borrar_todo(&st.db)?;
    }
    st.amigos.ya();
    let e = estado_inicial(&st);
    publicar(&app, e.clone());
    Ok(e)
}

/// Sincroniza ya (el botón de recargar, al abrir la pantalla de amigos).
#[tauri::command]
pub async fn amigos_sincronizar(app: AppHandle) -> AppResult<EstadoAmigos> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    sincronizar::sincronizar(&app).await?;
    Ok(st.amigos.estado())
}

/// Pide amistad con un código (`TAP-XXXX-XXXX`). Devuelve `pending` o `accepted` (si el otro
/// ya te la había pedido).
#[tauri::command]
pub async fn amigos_pedir(app: AppHandle, codigo: String) -> AppResult<String> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    // Vale el código suelto o el enlace entero (pegado o leído de un QR).
    let Some(codigo) = crate::enlace::codigo_amigo_pegado(&codigo) else {
        return Err(AppError::new(ErrorCode::InvalidInput).with("motivo", "codigo"));
    };
    let r = st
        .amigos
        .cliente()?
        .pedir_amistad(&st.red.clave(), &codigo)
        .await?;
    st.amigos.ya();
    Ok(r.status)
}

/// El QR de mi código de amigo: el enlace de la web, que cualquier cámara abre (y la app lee).
#[tauri::command]
pub fn amigos_qr(app: AppHandle) -> AppResult<Option<String>> {
    let st = app.state::<AppState>();
    let Some(codigo) = sincronizar::estado_inicial(&st).codigo else {
        return Ok(None);
    };
    crate::enlace::enlace_amigo(codigo)
        .map(|e| crate::pasar::encontrar::qr_svg(&e))
        .transpose()
}

#[tauri::command]
pub async fn amigos_responder(app: AppHandle, id: String, aceptar: bool) -> AppResult<()> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    st.amigos
        .cliente()?
        .responder(&st.red.clave(), &id, aceptar)
        .await?;
    st.amigos.ya();
    Ok(())
}

/// Quita a un amigo (y borra aquí la conversación).
#[tauri::command]
pub async fn amigos_quitar(app: AppHandle, clave: String) -> AppResult<()> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    st.amigos.cliente()?.quitar(&st.red.clave(), &clave).await?;
    almacen::olvidar(&st.db, &clave)?;
    st.amigos.ya();
    Ok(())
}

/// Bloquea (quita la amistad y borra aquí la conversación) o desbloquea.
#[tauri::command]
pub async fn amigos_bloquear(app: AppHandle, clave: String, bloquear: bool) -> AppResult<()> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    st.amigos
        .cliente()?
        .bloquear(&st.red.clave(), &clave, bloquear)
        .await?;
    if bloquear {
        almacen::olvidar(&st.db, &clave)?;
    }
    st.amigos.ya();
    Ok(())
}

/// Denuncia (con motivo; nunca con mensajes salvo lo que la persona pegue en la nota).
#[tauri::command]
pub async fn amigos_denunciar(
    app: AppHandle,
    clave: String,
    motivo: String,
    nota: Option<String>,
) -> AppResult<()> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    if !["spam", "abuse", "impersonation", "cheating", "other"].contains(&motivo.as_str()) {
        return Err(AppError::new(ErrorCode::InvalidInput).with("motivo", "motivo"));
    }
    let nota = nota
        .map(|n| n.chars().take(2000).collect::<String>())
        .filter(|n| !n.trim().is_empty());
    st.amigos
        .cliente()?
        .denunciar(&st.red.clave(), &clave, &motivo, nota.as_deref())
        .await
}

/// Un código de amigo nuevo (el viejo deja de valer).
#[tauri::command]
pub async fn amigos_nuevo_codigo(app: AppHandle) -> AppResult<String> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    let c = st.amigos.cliente()?.nuevo_codigo(&st.red.clave()).await?;
    settings::set(&st.db, CODIGO, &c)?;
    st.amigos.ya();
    Ok(c)
}

/// Lo que devuelve mandar: el mensaje tal como quedó y, si el servidor dijo que no, por qué.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Envio {
    pub mensaje: Mensaje,
    pub error: Option<AppError>,
}

async fn mandar(app: &AppHandle, clave: &str, contenido: Contenido) -> AppResult<Envio> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    let amigo = st
        .amigos
        .amigos
        .lock()
        .ok()
        .and_then(|a| a.get(clave).cloned())
        .ok_or_else(|| AppError::new(ErrorCode::Server).with("code", "not_friends"))?;
    let yo = st.red.clave();
    let carta = Carta {
        contenido,
        enviado: ahora_ms(),
    };
    // Antes de guardarlo: que se pueda cifrar y quepa.
    let para = cifrado::clave_de_amigo(
        &amigo.key,
        &amigo.encryption_key,
        &amigo.encryption_key_signature,
    )
    .ok_or_else(|| AppError::new(ErrorCode::Server).with("code", "bad_encryption_key"))?;
    cifrado::cerrar(&yo, &para, &carta)?;
    let mut id_cliente = [0u8; 16];
    getrandom::fill(&mut id_cliente)
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))?;
    let id_cliente = crate::amigos::firma::hex(&id_cliente);
    let id = almacen::guardar_mio(&st.db, clave, &carta, &id_cliente, ahora_ms())?;
    let error = sincronizar::mandar_guardado(&st, &yo, &amigo, &carta, &id_cliente)
        .await
        .err();
    let mensaje = almacen::mio(&st.db, id)?
        .map(|(m, _)| m)
        .ok_or_else(|| AppError::new(ErrorCode::Internal).with("detalle", "mensaje perdido"))?;
    Ok(Envio { mensaje, error })
}

#[tauri::command]
pub async fn amigos_mandar(app: AppHandle, clave: String, texto: String) -> AppResult<Envio> {
    let texto = texto.trim().to_string();
    if texto.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput).with("motivo", "vacio"));
    }
    mandar(&app, &clave, Contenido::Texto { texto }).await
}

/// Invita a una partida: manda el código directo que lleva solo el relé (`codigoMensaje`),
/// nunca el del QR con las IP (plan D5).
#[tauri::command]
pub async fn amigos_invitar(app: AppHandle, clave: String, codigo: String) -> AppResult<Envio> {
    // Sin las IP del anfitrión, aunque la pantalla mande el código entero.
    let codigo = crate::red::codigo::solo_rele(codigo.trim())?;
    mandar(&app, &clave, Contenido::Invitacion { codigo }).await
}

/// Vuelve a mandar uno que falló (con el mismo `clientId`: el servidor no lo duplica).
#[tauri::command]
pub async fn amigos_reintentar(app: AppHandle, id: i64) -> AppResult<Envio> {
    let st = app.state::<AppState>();
    exigir_activo(&st)?;
    let (m, id_cliente) =
        almacen::mio(&st.db, id)?.ok_or_else(|| AppError::new(ErrorCode::NotFound))?;
    let amigo = st
        .amigos
        .amigos
        .lock()
        .ok()
        .and_then(|a| a.get(&m.amigo).cloned())
        .ok_or_else(|| AppError::new(ErrorCode::Server).with("code", "not_friends"))?;
    almacen::poner_estado(&st.db, &id_cliente, "pendiente")?;
    let carta = Carta {
        contenido: if m.tipo == "invitacion" {
            Contenido::Invitacion { codigo: m.texto }
        } else {
            Contenido::Texto { texto: m.texto }
        },
        enviado: m.enviado,
    };
    let error = sincronizar::mandar_guardado(&st, &st.red.clave(), &amigo, &carta, &id_cliente)
        .await
        .err();
    let mensaje = almacen::mio(&st.db, id)?
        .map(|(m, _)| m)
        .ok_or_else(|| AppError::new(ErrorCode::NotFound))?;
    Ok(Envio { mensaje, error })
}

/// La conversación con un amigo (los últimos 50, o los 50 anteriores a `antes`).
#[tauri::command]
pub fn amigos_conversacion(
    state: State<'_, AppState>,
    clave: String,
    antes: Option<i64>,
) -> AppResult<Vec<Mensaje>> {
    almacen::conversacion(&state.db, &clave, antes, 50)
}

/// La conversación se ha visto: deja de contar como sin leer.
#[tauri::command]
pub fn amigos_leido(app: AppHandle, clave: String) -> AppResult<()> {
    let st = app.state::<AppState>();
    almacen::marcar_leido(&st.db, &clave)?;
    let mut e = st.amigos.estado();
    if let Some(a) = e.amigos.iter_mut().find(|a| a.clave == clave) {
        a.sin_leer = 0;
    }
    publicar(&app, e);
    Ok(())
}

/// Se comprobó (o se retira) el número de seguridad. También quita el aviso de clave cambiada.
#[tauri::command]
pub fn amigos_verificar(app: AppHandle, clave: String, verificado: bool) -> AppResult<()> {
    let st = app.state::<AppState>();
    almacen::verificar(&st.db, &clave, verificado)?;
    let mut e = st.amigos.estado();
    if let Some(a) = e.amigos.iter_mut().find(|a| a.clave == clave) {
        a.verificado = verificado;
        a.clave_cambio = false;
    }
    publicar(&app, e);
    Ok(())
}

/// Pone (o quita, con vacío) el nombre con el que veo a un amigo y mi nota sobre él, solo en este
/// equipo. Devuelve el estado con el cambio ya puesto.
#[tauri::command]
pub fn amigos_personal(
    app: AppHandle,
    st: State<'_, AppState>,
    clave: String,
    alias: String,
    nota: String,
) -> AppResult<EstadoAmigos> {
    let queda = almacen::poner_personal(&st.db, &clave, &alias, &nota)?;
    // Para los avisos con la app cerrada (Android), que no leen la base de datos.
    if cfg!(target_os = "android") {
        let todos = almacen::alias(&st.db)?;
        if let Err(e) = crate::amigos::fondo::guardar_alias(&st.paths.data, &todos) {
            tracing::info!(codigo = ?e.code, "amigos: no se guardaron los nombres para los avisos");
        }
    }
    let mut e = st.amigos.estado();
    if let Some(a) = e.amigos.iter_mut().find(|a| a.clave == clave) {
        a.alias = queda.alias;
        a.nota = queda.nota;
    }
    publicar(&app, e.clone());
    Ok(e)
}

/// «Estoy escribiendo» a un amigo: el servidor se lo pasa al momento si está conectado. La
/// pantalla lo manda como mucho cada 3 s mientras se escribe; un fallo no importa.
#[tauri::command]
pub async fn amigos_escribiendo(app: AppHandle, clave: String) {
    let st = app.state::<AppState>();
    if !activo(&st) {
        return;
    }
    let yo = st.red.clave();
    if let Ok(c) = st.amigos.cliente() {
        if let Err(e) = c.escribiendo(&yo, &clave).await {
            tracing::debug!(codigo = ?e.code, "amigos: no se pudo avisar de que se escribe");
        }
    }
}

/// El ritmo de `sync`: cada 10 s con un chat abierto, cada 60 s si no.
#[tauri::command]
pub fn amigos_ritmo(state: State<'_, AppState>, chat_abierto: bool) {
    state.amigos.ritmo(chat_abierto);
}

/// Los textos de los avisos del sistema, ya traducidos (Rust no escribe prosa).
#[tauri::command]
pub fn amigos_textos(st: State<'_, AppState>, textos: TextosAvisos) {
    // También para los avisos con la app cerrada (`amigos::fondo`), que no tienen la pantalla.
    if cfg!(target_os = "android") {
        if let Err(e) = crate::amigos::fondo::guardar_textos(&st.paths.data, &textos) {
            tracing::info!(codigo = ?e.code, "amigos: no se guardaron los textos de los avisos");
        }
    }
    avisar::poner_textos(textos);
}

/// La política de privacidad del servidor, en el navegador del sistema (la vista web no sale a
/// internet). La enlaza la pantalla de consentimiento (D0).
#[tauri::command]
pub fn amigos_privacidad(app: AppHandle) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let url = format!("{}/privacy", crate::amigos::cliente::base());
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))
}
