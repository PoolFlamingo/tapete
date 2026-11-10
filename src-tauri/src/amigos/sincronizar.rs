//! **El bucle de `sync`** (plan D1): al momento cuando llega un aviso por `/events` (`directo.rs`),
//! cada 60 s con la app abierta, cada 10 s con un chat abierto si `/events` no está conectado,
//! nada en segundo plano en el móvil, y con espera creciente si el servidor falla.
//!
//! Cada vuelta: `hello` si cambió el apodo, la versión o la clave; reintenta lo que se quedó sin
//! mandar; pide `sync`, descifra y guarda los mensajes, los confirma (`ack`: el servidor los
//! borra), apunta los acuses de entrega y deja el estado para la pantalla (`amigos://estado`).
//! Los mensajes nuevos van en `amigos://mensajes` (las claves de los amigos que escribieron).

use std::sync::atomic::Ordering;
use std::time::Duration;

use base64::Engine;
use iroh::SecretKey;
use tauri::{AppHandle, Emitter, Manager};

use super::cifrado::{self, Carta, Contenido};
use super::cliente::{AmigoServidor, Hola, Sync};
use super::{ahora_ms, AmigoVista, AvisoVista, EstadoAmigos, SolicitudVista};
use super::{ACTIVO, CODIGO, CURSOR, HOLA};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::state::AppState;
use crate::store::{amigos as almacen, settings};

const NORMAL: Duration = Duration::from_secs(60);
const CON_CHAT: Duration = Duration::from_secs(10);
const MAX_ESPERA: Duration = Duration::from_secs(600);

pub fn activo(st: &AppState) -> bool {
    settings::get(&st.db, ACTIVO).ok().flatten().as_deref() == Some("1")
}

fn apodo(app: &AppHandle) -> String {
    let a = crate::pasar::nombre_equipo(app);
    if a.trim().is_empty() {
        "Tapete".to_string()
    } else {
        a.chars().take(32).collect()
    }
}

/// Un ajuste de amigos; vacío es como no tenerlo (así se borran al salir o al cambiar de clave).
fn leer(st: &AppState, clave: &str) -> AppResult<Option<String>> {
    Ok(settings::get(&st.db, clave)?.filter(|v| !v.is_empty()))
}

/// `hello` si cambió algo de lo que se publica (o nunca se hizo).
async fn presentarse(app: &AppHandle, st: &AppState, clave: &SecretKey) -> AppResult<()> {
    let (cifrado_hex, firma_hex) = cifrado::clave_firmada(clave);
    let apodo = apodo(app);
    let version = app.package_info().version.to_string();
    let huella = format!(
        "{apodo}\n{version}\n{cifrado_hex}\n{}",
        super::cliente::base()
    );
    if settings::get(&st.db, HOLA)?.as_deref() == Some(huella.as_str())
        && leer(st, CODIGO)?.is_some()
    {
        return Ok(());
    }
    let r = st
        .amigos
        .cliente()?
        .hola(
            clave,
            &Hola {
                nickname: &apodo,
                platform: super::cliente::plataforma(),
                version: &version,
                encryption_key: &cifrado_hex,
                encryption_key_signature: &firma_hex,
            },
        )
        .await?;
    settings::set(&st.db, CODIGO, &r.friend_code)?;
    settings::set(&st.db, HOLA, &huella)?;
    Ok(())
}

/// Manda (o vuelve a mandar) un mensaje ya guardado. `true` si el servidor lo tiene.
pub(crate) async fn mandar_guardado(
    st: &AppState,
    clave: &SecretKey,
    amigo: &AmigoServidor,
    carta: &Carta,
    id_cliente: &str,
) -> AppResult<bool> {
    let Some(para) = cifrado::clave_de_amigo(
        &amigo.key,
        &amigo.encryption_key,
        &amigo.encryption_key_signature,
    ) else {
        almacen::poner_estado(&st.db, id_cliente, "fallido")?;
        return Err(AppError::new(ErrorCode::Server).with("code", "bad_encryption_key"));
    };
    let datos = cifrado::cerrar(clave, &para, carta)?;
    match st
        .amigos
        .cliente()?
        .mandar(clave, &amigo.key, &datos, id_cliente)
        .await
    {
        Ok(r) => {
            almacen::enviado(&st.db, id_cliente, &r.id)?;
            Ok(true)
        }
        // Sin red: se queda pendiente y lo reintenta la siguiente vuelta.
        Err(e) if e.code == ErrorCode::Network => Ok(false),
        // El servidor dijo que no (no sois amigos, bandeja llena…): no se reintenta solo.
        Err(e) => {
            almacen::poner_estado(&st.db, id_cliente, "fallido")?;
            Err(e)
        }
    }
}

fn carta_de(m: &almacen::Mensaje) -> Carta {
    Carta {
        contenido: if m.tipo == "invitacion" {
            Contenido::Invitacion {
                codigo: m.texto.clone(),
            }
        } else {
            Contenido::Texto {
                texto: m.texto.clone(),
            }
        },
        enviado: m.enviado,
    }
}

/// Lo que se hace con una respuesta de `sync`: devuelve los amigos con mensajes nuevos y los
/// ids que hay que confirmar.
pub(crate) fn procesar(
    st: &AppState,
    clave: &SecretKey,
    s: &Sync,
) -> AppResult<(Vec<String>, Vec<String>)> {
    let mut nuevos: Vec<String> = Vec::new();
    let mut confirmar = Vec::new();
    for m in &s.messages {
        confirmar.push(m.id.clone());
        let Some(amigo) = s.friends.iter().find(|f| f.key == m.from) else {
            continue;
        };
        let Some(de) = cifrado::clave_de_amigo(
            &amigo.key,
            &amigo.encryption_key,
            &amigo.encryption_key_signature,
        ) else {
            tracing::warn!("amigos: mensaje de alguien con la clave de cifrado sin firmar bien");
            continue;
        };
        let Ok(datos) = base64::engine::general_purpose::STANDARD.decode(&m.ciphertext) else {
            continue;
        };
        let Some(carta) = cifrado::abrir(clave, &de, &datos) else {
            tracing::warn!("amigos: un mensaje no se pudo abrir (alterado o de otra clave)");
            continue;
        };
        if almacen::guardar_recibido(&st.db, &m.from, &carta, &m.id, ahora_ms())?
            && !nuevos.contains(&m.from)
        {
            nuevos.push(m.from.clone());
        }
    }
    for e in &s.deliveries {
        if let Some(c) = &e.client_id {
            almacen::poner_estado(&st.db, c, "entregado")?;
        }
    }
    Ok((nuevos, confirmar))
}

/// El estado para la pantalla a partir de la última respuesta.
pub(crate) fn vista(st: &AppState, clave: &SecretKey, s: &Sync) -> AppResult<EstadoAmigos> {
    let sin_leer = almacen::no_leidos(&st.db)?;
    let personal = almacen::personal(&st.db)?;
    let yo = firma_hex(clave);
    let (mi_cifrado, _) = cifrado::clave_firmada(clave);
    let mut amigos = Vec::new();
    for f in &s.friends {
        let valida =
            cifrado::clave_de_amigo(&f.key, &f.encryption_key, &f.encryption_key_signature)
                .is_some();
        let confianza = if valida {
            almacen::anotar_clave(&st.db, &f.key, &f.encryption_key)?
        } else {
            almacen::Confianza::default()
        };
        amigos.push(AmigoVista {
            clave: f.key.clone(),
            apodo: f.nickname.clone(),
            alias: personal.get(&f.key).and_then(|p| p.alias.clone()),
            nota: personal.get(&f.key).and_then(|p| p.nota.clone()),
            en_linea: f.online,
            visto: f.last_seen.clone(),
            clave_valida: valida,
            verificado: confianza.verificado,
            clave_cambio: confianza.cambio,
            numero_seguridad: cifrado::numero_de_seguridad(
                (&yo, &mi_cifrado),
                (&f.key, &f.encryption_key),
            ),
            sin_leer: sin_leer.get(&f.key).copied().unwrap_or(0),
        });
    }
    let solicitud = |r: &super::cliente::Solicitud| SolicitudVista {
        id: r.id.clone(),
        clave: r.key.clone(),
        apodo: r.nickname.clone(),
        creada: r.created_at.clone(),
    };
    Ok(EstadoAmigos {
        activo: true,
        codigo: Some(s.me.friend_code.clone()).filter(|c| !c.is_empty()),
        clave: yo,
        amigos,
        recibidas: s.requests.incoming.iter().map(solicitud).collect(),
        enviadas: s.requests.outgoing.iter().map(solicitud).collect(),
        bloqueados: s.blocked.clone(),
        avisos: s
            .notices
            .iter()
            .map(|a| AvisoVista {
                id: a.id.clone(),
                nivel: a.level.clone(),
                textos: a.text.clone(),
            })
            .collect(),
        error: None,
        sincronizado: Some(ahora_ms()),
    })
}

pub(crate) fn firma_hex(clave: &SecretKey) -> String {
    super::firma::hex(clave.public().as_bytes())
}

/// Lo que ve la pantalla sin haber sincronizado todavía (o tras salir).
pub fn estado_inicial(st: &AppState) -> EstadoAmigos {
    EstadoAmigos {
        activo: activo(st),
        codigo: leer(st, CODIGO).ok().flatten(),
        clave: firma_hex(&st.red.clave()),
        ..EstadoAmigos::default()
    }
}

pub(crate) fn publicar(app: &AppHandle, e: EstadoAmigos) {
    if let Some(st) = app.try_state::<AppState>() {
        if let Ok(mut g) = st.amigos.estado.lock() {
            *g = e.clone();
        }
    }
    let _ = app.emit("amigos://estado", e);
}

/// Una vuelta entera. Devuelve los amigos que escribieron algo nuevo.
pub async fn sincronizar(app: &AppHandle) -> AppResult<Vec<String>> {
    let st = app.state::<AppState>();
    if !activo(&st) {
        return Ok(Vec::new());
    }
    let _turno = st.amigos.turno.lock().await;
    let clave = st.red.clave();
    presentarse(app, &st, &clave).await?;

    // Lo que se quedó sin mandar por falta de red.
    let pendientes = almacen::pendientes(&st.db)?;
    if !pendientes.is_empty() {
        let amigos = st
            .amigos
            .amigos
            .lock()
            .map(|a| a.clone())
            .unwrap_or_default();
        for (m, id_cliente) in pendientes {
            if let Some(a) = amigos.get(&m.amigo) {
                let _ = mandar_guardado(&st, &clave, a, &carta_de(&m), &id_cliente).await;
            }
        }
    }

    let mut nuevos: Vec<String> = Vec::new();
    let mut cursor = leer(&st, CURSOR)?;
    let mut ultima: Option<Sync> = None;
    // Con más de 100 pendientes, el servidor dice `moreMessages`: se piden más tras confirmar.
    for _ in 0..10 {
        let s = st.amigos.cliente()?.sync(&clave, cursor.as_deref()).await?;
        if let Ok(mut a) = st.amigos.amigos.lock() {
            *a = s
                .friends
                .iter()
                .map(|f| (f.key.clone(), f.clone()))
                .collect();
        }
        let (n, confirmar) = procesar(&st, &clave, &s)?;
        for k in n {
            if !nuevos.contains(&k) {
                nuevos.push(k);
            }
        }
        if !confirmar.is_empty() {
            st.amigos.cliente()?.confirmar(&clave, &confirmar).await?;
        }
        settings::set(&st.db, CURSOR, &s.cursor)?;
        cursor = Some(s.cursor.clone());
        let mas = s.more_messages;
        ultima = Some(s);
        if !mas {
            break;
        }
    }
    if let Some(s) = ultima {
        publicar(app, vista(&st, &clave, &s)?);
    }
    if !nuevos.is_empty() {
        let _ = app.emit("amigos://mensajes", &nuevos);
        super::avisar::mensajes_nuevos(app, &nuevos);
    }
    Ok(nuevos)
}

/// La identidad de la app ha cambiado (`red_identidad_importar`): para el servidor es otra
/// persona, con su código, sus amigos y su cursor. Se olvida lo de la clave anterior (las
/// conversaciones guardadas se quedan) y se presenta de nuevo en la siguiente vuelta.
pub async fn otra_clave(app: &AppHandle) -> AppResult<()> {
    let st = app.state::<AppState>();
    // Tras la vuelta que esté en marcha, que aún escribe el cursor de la clave anterior.
    let _turno = st.amigos.turno.lock().await;
    for k in [CODIGO, CURSOR, HOLA] {
        settings::set(&st.db, k, "")?;
    }
    if let Ok(mut a) = st.amigos.amigos.lock() {
        a.clear();
    }
    publicar(app, estado_inicial(&st));
    // `/events` va firmada con la clave: se abre otra vez con la nueva.
    st.amigos.cortar.notify_waiters();
    st.amigos.ya();
    Ok(())
}

/// Arranca el bucle (y la escucha en directo) si la persona aceptó usar el servidor y no está ya
/// en marcha.
pub fn arrancar(app: AppHandle) {
    super::directo::arrancar(app.clone());
    let st = app.state::<AppState>();
    if !activo(&st) || st.amigos.en_marcha.swap(true, Ordering::SeqCst) {
        return;
    }
    // Con la app cerrada, WorkManager (D4b). Programarlo otra vez no lo duplica.
    if let Err(e) = crate::android::avisos_amigos(&app, Some(&st.paths.data)) {
        tracing::info!(codigo = ?e.code, "amigos: no se programaron los avisos en segundo plano");
    }
    tauri::async_runtime::spawn(async move {
        let mut fallos: u32 = 0;
        let mut espera = Duration::ZERO;
        loop {
            {
                let st = app.state::<AppState>();
                tokio::select! {
                    _ = tokio::time::sleep(espera) => {}
                    _ = st.amigos.despertar.notified() => {}
                }
                if !activo(&st) {
                    st.amigos.en_marcha.store(false, Ordering::SeqCst);
                    return;
                }
                if st.amigos.en_segundo_plano.load(Ordering::SeqCst) {
                    espera = NORMAL;
                    continue;
                }
            }
            let r = sincronizar(&app).await;
            let st = app.state::<AppState>();
            match r {
                Ok(_) => {
                    fallos = 0;
                    // Con `/events` conectado, lo nuevo llega al momento: no hace falta correr.
                    espera = if st.amigos.chat_abierto.load(Ordering::SeqCst)
                        && !st.amigos.en_directo.load(Ordering::SeqCst)
                    {
                        CON_CHAT
                    } else {
                        NORMAL
                    };
                }
                Err(e) => {
                    fallos = fallos.saturating_add(1);
                    // 60 s, 120 s, 240 s, 480 s y después 10 min.
                    espera = NORMAL
                        .saturating_mul(1 << fallos.min(5).saturating_sub(1))
                        .min(MAX_ESPERA);
                    tracing::info!(codigo = ?e.code, ?espera, "amigos: sync falló");
                    let mut actual = st.amigos.estado();
                    actual.activo = true;
                    actual.error = Some(e);
                    publicar(&app, actual);
                }
            }
        }
    });
}
