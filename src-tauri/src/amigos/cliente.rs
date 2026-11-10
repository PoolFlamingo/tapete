//! **El cliente del API de Tapete** (`/api/v1/`, contrato v1 en `docs/api.md` del servidor): cada
//! ruta, firmada (`firma.rs`), con el cuerpo en JSON. El cliente HTTP sale de `tls.rs` (el TLS
//! va en un solo sitio) y los fallos de red se montan con `error::red`/`error::estado_http`.
//!
//! Un error del servidor (`{ code, params }`) llega a la pantalla como `ErrorCode::Server` con
//! `params.code` = el código del servidor (`not_friends`, `friend_code_not_found`…) y sus datos.

use std::time::Duration;

use iroh::SecretKey;
use reqwest::{Client, Method};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::firma;
use crate::error::{self, AppError, AppResult, ErrorCode};

/// El servidor de producción. En depuración, `PB_TAPETE` lo cambia (por ejemplo, el servidor en
/// local: `PB_TAPETE=http://127.0.0.1:1530`): al arrancar o, para Android, donde la app no ve
/// las variables de quien la lanza, al compilar (`PB_TAPETE=http://10.0.2.2:1530` desde el
/// emulador).
pub const PRODUCCION: &str = "https://tapete.zodhub.app";

pub fn base() -> String {
    if cfg!(debug_assertions) {
        let b = std::env::var("PB_TAPETE")
            .ok()
            .or_else(|| option_env!("PB_TAPETE").map(str::to_string));
        if let Some(b) = b.filter(|b| !b.is_empty()) {
            return b.trim_end_matches('/').to_string();
        }
    }
    PRODUCCION.to_string()
}

/// La plataforma, con los nombres del API.
pub fn plataforma() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

// ── Lo que viaja (los nombres son los del contrato) ─────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hola<'a> {
    pub nickname: &'a str,
    pub platform: &'a str,
    pub version: &'a str,
    pub encryption_key: &'a str,
    pub encryption_key_signature: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HolaRespuesta {
    pub friend_code: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sync {
    pub cursor: String,
    pub me: Yo,
    #[serde(default)]
    pub friends: Vec<AmigoServidor>,
    #[serde(default)]
    pub requests: Solicitudes,
    #[serde(default)]
    pub blocked: Vec<String>,
    #[serde(default)]
    pub messages: Vec<MensajeServidor>,
    #[serde(default)]
    pub more_messages: bool,
    #[serde(default)]
    pub deliveries: Vec<Entrega>,
    #[serde(default)]
    pub notices: Vec<AvisoServidor>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Yo {
    pub friend_code: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmigoServidor {
    pub key: String,
    pub nickname: String,
    pub encryption_key: String,
    pub encryption_key_signature: String,
    #[serde(default)]
    pub online: bool,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Solicitudes {
    #[serde(default)]
    pub incoming: Vec<Solicitud>,
    #[serde(default)]
    pub outgoing: Vec<Solicitud>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solicitud {
    pub id: String,
    pub key: String,
    pub nickname: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MensajeServidor {
    pub id: String,
    pub from: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entrega {
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvisoServidor {
    pub id: String,
    pub level: String,
    #[serde(default)]
    pub text: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeticionAmistad {
    pub status: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enviado {
    pub id: String,
}

#[derive(Deserialize)]
struct ErrorServidor {
    code: String,
    #[serde(default)]
    params: std::collections::BTreeMap<String, serde_json::Value>,
}

// ── El cliente ─────────────────────────────────────────────────────────────

pub struct Cliente {
    http: Client,
    base: String,
}

impl Cliente {
    pub fn nuevo(base: String) -> AppResult<Cliente> {
        let http = crate::tls::reqwest(Client::builder())
            .user_agent(crate::tls::AGENTE)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(error::red)?;
        Ok(Cliente { http, base })
    }

    async fn pedir<T: DeserializeOwned>(
        &self,
        clave: &SecretKey,
        metodo: Method,
        ruta: &str,
        cuerpo: Option<&impl Serialize>,
    ) -> AppResult<T> {
        let cuerpo = match cuerpo {
            Some(c) => serde_json::to_vec(c)
                .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))?,
            None => Vec::new(),
        };
        let ruta_firmada = format!("/api/v1{ruta}");
        let h = firma::firmar(clave, metodo.as_str(), &ruta_firmada, &cuerpo);
        let mut rb = self
            .http
            .request(metodo, format!("{}{ruta_firmada}", self.base))
            .header("X-Tapete-Key", h.clave)
            .header("X-Tapete-Timestamp", h.timestamp)
            .header("X-Tapete-Nonce", h.nonce)
            .header("X-Tapete-Signature", h.firma);
        if !cuerpo.is_empty() {
            rb = rb.header("Content-Type", "application/json").body(cuerpo);
        }
        let r = rb.send().await.map_err(error::red)?;
        let estado = r.status();
        let bytes = r.bytes().await.map_err(error::red)?;
        if estado.is_success() {
            return serde_json::from_slice(&bytes).map_err(|e| {
                AppError::new(ErrorCode::Network)
                    .with("motivo", "respuesta")
                    .with("detalle", e)
            });
        }
        match serde_json::from_slice::<ErrorServidor>(&bytes) {
            Ok(e) => {
                tracing::info!(code = %e.code, estado = estado.as_u16(), "amigos: el servidor dice que no");
                let mut err = AppError::new(ErrorCode::Server).with("code", e.code);
                for (k, v) in e.params {
                    let v = match v {
                        serde_json::Value::String(s) => s,
                        otro => otro.to_string(),
                    };
                    err = err.with(&k, v);
                }
                Err(err)
            }
            Err(_) => Err(error::estado_http(estado)),
        }
    }

    pub async fn hola(&self, clave: &SecretKey, h: &Hola<'_>) -> AppResult<HolaRespuesta> {
        self.pedir(clave, Method::POST, "/hello", Some(h)).await
    }

    pub async fn sync(&self, clave: &SecretKey, cursor: Option<&str>) -> AppResult<Sync> {
        // El cursor es base64url: viaja tal cual en la query (lo firmado es lo que viaja).
        let ruta = match cursor.filter(|c| {
            !c.is_empty()
                && c.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) {
            Some(c) => format!("/sync?since={c}"),
            None => "/sync".to_string(),
        };
        self.pedir(clave, Method::GET, &ruta, None::<&()>).await
    }

    pub async fn pedir_amistad(
        &self,
        clave: &SecretKey,
        codigo: &str,
    ) -> AppResult<PeticionAmistad> {
        #[derive(Serialize)]
        struct C<'a> {
            code: &'a str,
        }
        self.pedir(
            clave,
            Method::POST,
            "/friends/requests",
            Some(&C { code: codigo }),
        )
        .await
    }

    fn id_valido(id: &str) -> AppResult<&str> {
        if !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            Ok(id)
        } else {
            Err(AppError::new(ErrorCode::InvalidInput).with("motivo", "id"))
        }
    }

    pub async fn responder(&self, clave: &SecretKey, id: &str, aceptar: bool) -> AppResult<()> {
        let accion = if aceptar { "accept" } else { "decline" };
        let ruta = format!("/friends/requests/{}/{accion}", Self::id_valido(id)?);
        let _: serde_json::Value = self.pedir(clave, Method::POST, &ruta, None::<&()>).await?;
        Ok(())
    }

    pub async fn quitar(&self, clave: &SecretKey, amigo: &str) -> AppResult<()> {
        let ruta = format!("/friends/{}", Self::id_valido(amigo)?);
        let _: serde_json::Value = self
            .pedir(clave, Method::DELETE, &ruta, None::<&()>)
            .await?;
        Ok(())
    }

    pub async fn mandar(
        &self,
        clave: &SecretKey,
        para: &str,
        cifrado: &[u8],
        id_cliente: &str,
    ) -> AppResult<Enviado> {
        use base64::Engine;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct M<'a> {
            to: &'a str,
            ciphertext: String,
            client_id: &'a str,
        }
        let m = M {
            to: para,
            ciphertext: base64::engine::general_purpose::STANDARD.encode(cifrado),
            client_id: id_cliente,
        };
        self.pedir(clave, Method::POST, "/messages", Some(&m)).await
    }

    pub async fn confirmar(&self, clave: &SecretKey, ids: &[String]) -> AppResult<()> {
        #[derive(Serialize)]
        struct A<'a> {
            ids: &'a [String],
        }
        for trozo in ids.chunks(200) {
            let _: serde_json::Value = self
                .pedir(
                    clave,
                    Method::POST,
                    "/messages/ack",
                    Some(&A { ids: trozo }),
                )
                .await?;
        }
        Ok(())
    }

    pub async fn bloquear(&self, clave: &SecretKey, quien: &str, si: bool) -> AppResult<()> {
        #[derive(Serialize)]
        struct B<'a> {
            key: &'a str,
        }
        let _: serde_json::Value = if si {
            self.pedir(clave, Method::POST, "/blocks", Some(&B { key: quien }))
                .await?
        } else {
            let ruta = format!("/blocks/{}", Self::id_valido(quien)?);
            self.pedir(clave, Method::DELETE, &ruta, None::<&()>)
                .await?
        };
        Ok(())
    }

    pub async fn denunciar(
        &self,
        clave: &SecretKey,
        quien: &str,
        motivo: &str,
        nota: Option<&str>,
    ) -> AppResult<()> {
        #[derive(Serialize)]
        struct D<'a> {
            key: &'a str,
            reason: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<&'a str>,
        }
        let _: serde_json::Value = self
            .pedir(
                clave,
                Method::POST,
                "/reports",
                Some(&D {
                    key: quien,
                    reason: motivo,
                    note: nota,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn nuevo_codigo(&self, clave: &SecretKey) -> AppResult<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct C {
            friend_code: String,
        }
        let c: C = self
            .pedir(clave, Method::POST, "/me/code", None::<&()>)
            .await?;
        Ok(c.friend_code)
    }

    pub async fn borrarme(&self, clave: &SecretKey) -> AppResult<()> {
        let _: serde_json::Value = self
            .pedir(clave, Method::DELETE, "/me", None::<&()>)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_plataforma_es_una_de_las_del_api() {
        assert!(["linux", "windows", "macos", "android", "ios", "other"].contains(&plataforma()));
    }

    #[test]
    fn un_id_raro_no_sale_en_la_ruta() {
        assert!(Cliente::id_valido("abc-123_X").is_ok());
        for malo in ["", "../me", "a/b", "a?b", "a b", &"x".repeat(65)] {
            assert!(Cliente::id_valido(malo).is_err(), "{malo}");
        }
    }

    #[test]
    fn la_respuesta_de_sync_se_lee_con_lo_que_falte() {
        let s: Sync = serde_json::from_str(
            r#"{"serverTime":"t","cursor":"Yw","me":{"friendCode":"TAP-AAAA-BBBB"},
                "friends":[{"key":"k","nickname":"Ana","encryptionKey":"e",
                "encryptionKeySignature":"f","online":true,"lastSeen":"t","since":"t"}],
                "requests":{"incoming":[],"outgoing":[]},"blocked":[],"messages":[],
                "moreMessages":false,"deliveries":[{"messageId":"m","clientId":"c","to":"k",
                "deliveredAt":"t"}],"notices":[]}"#,
        )
        .unwrap();
        assert_eq!(s.me.friend_code, "TAP-AAAA-BBBB");
        assert_eq!(s.friends[0].nickname, "Ana");
        assert_eq!(s.deliveries[0].client_id.as_deref(), Some("c"));
    }
}
