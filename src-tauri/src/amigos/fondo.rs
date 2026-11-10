//! **Avisos con la app cerrada** (plan Tapete, D4b): en Android, WorkManager despierta cada ~15
//! minutos el `Worker` de `plugins/android-extra` (`AvisosAmigos.kt`), que abre la clave con el
//! cofre del Keystore y llama aquí por JNI (`Nativo.comprobar`).
//!
//! 🛑 Aquí **solo se mira**: `GET /sync` sin cursor, sin `ack` y sin tocar la base de datos. Los
//! mensajes se quedan en el servidor hasta que la app se abre y los recoge como siempre (el
//! bucle de `sincronizar.rs`); por eso un aviso de aquí nunca se pierde ni se duplica en la
//! conversación. Se descifra solo para saber si es un mensaje o una invitación: el aviso es
//! «Mensaje de <apodo>», **nunca** el texto.
//!
//! Lo que se comparte con la app, en `app_data_dir/amigos/`:
//! - `textos-avisos.json`: los textos ya traducidos que manda la pantalla (`amigos_textos`). Rust
//!   no escribe prosa: sin ellos no se avisa.
//! - `avisados.json`: los ids de los mensajes ya avisados que siguen en el servidor (así un
//!   mensaje sin recoger no avisa cada 15 minutos).

use std::path::Path;

use base64::Engine;
use iroh::SecretKey;
use serde::{Deserialize, Serialize};

use super::avisar::{cuerpo, TextosAvisos};
use super::cifrado::{self, Contenido};
use super::cliente::{Cliente, Sync};
use crate::error::{AppError, AppResult, ErrorCode};

const CARPETA: &str = "amigos";
const TEXTOS: &str = "textos-avisos.json";
const AVISADOS: &str = "avisados.json";

/// Un aviso para el sistema, ya escrito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aviso {
    /// La identidad del amigo: un aviso por amigo (el siguiente sustituye al anterior).
    pub amigo: String,
    pub titulo: String,
    pub cuerpo: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Avisados {
    ids: Vec<String>,
}

/// Guarda los textos de los avisos para cuando la app esté cerrada.
pub fn guardar_textos(datos: &Path, t: &TextosAvisos) -> AppResult<()> {
    let carpeta = datos.join(CARPETA);
    std::fs::create_dir_all(&carpeta)?;
    let json =
        serde_json::to_vec(t).map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))?;
    std::fs::write(carpeta.join(TEXTOS), json)?;
    Ok(())
}

fn leer<T: for<'de> Deserialize<'de>>(ruta: &Path) -> Option<T> {
    std::fs::read(ruta)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

/// Lo que hay que avisar de esta respuesta de `sync`, y los ids que quedan avisados (solo los que
/// siguen en el servidor, así la lista nunca crece más que lo que hay sin recoger).
pub(crate) fn avisos_de(
    clave: &SecretKey,
    s: &Sync,
    textos: &TextosAvisos,
    avisados: &[String],
) -> (Vec<Aviso>, Vec<String>) {
    // Por amigo, si lo último nuevo es una invitación (como `avisar::mensajes_nuevos`).
    let mut por_amigo: Vec<(String, bool)> = Vec::new();
    let mut quedan = Vec::new();
    for m in &s.messages {
        if avisados.contains(&m.id) {
            quedan.push(m.id.clone());
            continue;
        }
        let Some(amigo) = s.friends.iter().find(|f| f.key == m.from) else {
            continue;
        };
        let Some(de) = cifrado::clave_de_amigo(
            &amigo.key,
            &amigo.encryption_key,
            &amigo.encryption_key_signature,
        ) else {
            continue;
        };
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&m.ciphertext) else {
            continue;
        };
        // Lo que no se abre no se avisa (la app lo descartará igual al recogerlo).
        let Some(carta) = cifrado::abrir(clave, &de, &bytes) else {
            continue;
        };
        quedan.push(m.id.clone());
        let invitacion = matches!(carta.contenido, Contenido::Invitacion { .. });
        match por_amigo.iter_mut().find(|(k, _)| *k == m.from) {
            Some(p) => p.1 = invitacion,
            None => por_amigo.push((m.from.clone(), invitacion)),
        }
    }
    let avisos = por_amigo
        .into_iter()
        .filter_map(|(k, invitacion)| {
            let apodo = &s.friends.iter().find(|f| f.key == k)?.nickname;
            Some(Aviso {
                titulo: textos.titulo.clone(),
                cuerpo: cuerpo(textos, apodo, invitacion),
                amigo: k,
            })
        })
        .collect();
    (avisos, quedan)
}

/// Una vuelta en segundo plano: mira el servidor y devuelve los avisos que hay que enseñar.
pub async fn comprobar(datos: &Path, clave: &SecretKey) -> AppResult<Vec<Aviso>> {
    let carpeta = datos.join(CARPETA);
    let Some(textos) = leer::<TextosAvisos>(&carpeta.join(TEXTOS)) else {
        return Ok(Vec::new());
    };
    let avisados = leer::<Avisados>(&carpeta.join(AVISADOS)).unwrap_or_default();
    let s = Cliente::nuevo(super::cliente::base())?
        .sync(clave, None)
        .await?;
    let (avisos, quedan) = avisos_de(clave, &s, &textos, &avisados.ids);
    if quedan != avisados.ids {
        let json = serde_json::to_vec(&Avisados { ids: quedan }).unwrap_or_default();
        std::fs::write(carpeta.join(AVISADOS), json)?;
    }
    Ok(avisos)
}

/// La entrada desde Kotlin: `Nativo.comprobar(datos, clave)` → JSON con los avisos, o `null` si
/// algo falló (se vuelve a probar en la siguiente vuelta).
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_estigio_proxybuilder_extra_Nativo_comprobar<'l>(
    mut env: jni::JNIEnv<'l>,
    _clase: jni::objects::JClass<'l>,
    datos: jni::objects::JString<'l>,
    clave: jni::objects::JByteArray<'l>,
) -> jni::sys::jstring {
    let Ok(datos) = env.get_string(&datos).map(String::from) else {
        return std::ptr::null_mut();
    };
    let Ok(mut bytes) = env.convert_byte_array(&clave) else {
        return std::ptr::null_mut();
    };
    let clave = <[u8; 32]>::try_from(bytes.as_slice())
        .ok()
        .map(|b| SecretKey::from_bytes(&b));
    bytes.fill(0);
    let Some(clave) = clave else {
        return std::ptr::null_mut();
    };
    // Un pánico no puede cruzar a Java: se queda en `null`.
    let r = std::panic::catch_unwind(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        match rt.block_on(comprobar(Path::new(&datos), &clave)) {
            Ok(a) => serde_json::to_string(&a).ok(),
            Err(e) => {
                tracing::info!(codigo = ?e.code, "amigos: la comprobación en segundo plano falló");
                None
            }
        }
    });
    match r.ok().flatten().and_then(|j| env.new_string(j).ok()) {
        Some(s) => s.into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::cifrado::Carta;
    use super::super::cliente::{AmigoServidor, MensajeServidor};
    use super::*;

    fn textos() -> TextosAvisos {
        TextosAvisos {
            titulo: "Tapete".into(),
            mensaje_de: "Mensaje de {apodo}".into(),
            invitacion_de: "{apodo} te invita a una partida".into(),
        }
    }

    fn amigo(clave: &SecretKey, apodo: &str) -> AmigoServidor {
        let (cifrado_hex, firma_hex) = cifrado::clave_firmada(clave);
        AmigoServidor {
            key: super::super::sincronizar::firma_hex(clave),
            nickname: apodo.into(),
            encryption_key: cifrado_hex,
            encryption_key_signature: firma_hex,
            online: false,
            last_seen: None,
        }
    }

    fn mensaje(
        id: &str,
        de: &SecretKey,
        para: &SecretKey,
        contenido: Contenido,
    ) -> MensajeServidor {
        let (cifrado_hex, firma_hex) = cifrado::clave_firmada(para);
        let destino = cifrado::clave_de_amigo(
            &super::super::sincronizar::firma_hex(para),
            &cifrado_hex,
            &firma_hex,
        )
        .unwrap();
        let datos = cifrado::cerrar(
            de,
            &destino,
            &Carta {
                contenido,
                enviado: 1,
            },
        )
        .unwrap();
        MensajeServidor {
            id: id.into(),
            from: super::super::sincronizar::firma_hex(de),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(datos),
        }
    }

    fn sync(friends: Vec<AmigoServidor>, messages: Vec<MensajeServidor>) -> Sync {
        Sync {
            friends,
            messages,
            ..Sync::default()
        }
    }

    #[test]
    fn avisa_una_vez_por_amigo_sin_el_texto_y_no_repite() {
        let yo = SecretKey::generate();
        let ana = SecretKey::generate();
        let bea = SecretKey::generate();
        let texto = |t: &str| Contenido::Texto { texto: t.into() };
        let s = sync(
            vec![amigo(&ana, "Ana"), amigo(&bea, "Bea")],
            vec![
                mensaje("1", &ana, &yo, texto("secreto")),
                mensaje("2", &ana, &yo, texto("otro secreto")),
                mensaje(
                    "3",
                    &bea,
                    &yo,
                    Contenido::Invitacion {
                        codigo: "PB1-x".into(),
                    },
                ),
            ],
        );
        let (avisos, quedan) = avisos_de(&yo, &s, &textos(), &[]);
        let cuerpos: Vec<_> = avisos.iter().map(|a| a.cuerpo.as_str()).collect();
        assert_eq!(cuerpos, ["Mensaje de Ana", "Bea te invita a una partida"]);
        assert!(avisos.iter().all(|a| !a.cuerpo.contains("secreto")));
        assert_eq!(quedan, ["1", "2", "3"]);

        // La siguiente vuelta, con lo mismo sin recoger, no avisa otra vez.
        let (avisos, quedan2) = avisos_de(&yo, &s, &textos(), &quedan);
        assert!(avisos.is_empty());
        assert_eq!(quedan2, quedan);

        // Lo recogido por la app (ya no está en el servidor) sale de la lista.
        let s = sync(
            vec![amigo(&ana, "Ana")],
            vec![mensaje("4", &ana, &yo, texto("hola"))],
        );
        let (avisos, quedan) = avisos_de(&yo, &s, &textos(), &quedan);
        assert_eq!(avisos.len(), 1);
        assert_eq!(quedan, ["4"]);
    }

    #[test]
    fn lo_que_no_se_abre_no_avisa() {
        let yo = SecretKey::generate();
        let ana = SecretKey::generate();
        let otro = SecretKey::generate();
        // Cifrado para otra persona: no se abre con mi clave.
        let s = sync(
            vec![amigo(&ana, "Ana")],
            vec![mensaje(
                "1",
                &ana,
                &otro,
                Contenido::Texto { texto: "x".into() },
            )],
        );
        let (avisos, quedan) = avisos_de(&yo, &s, &textos(), &[]);
        assert!(avisos.is_empty());
        assert!(quedan.is_empty());
    }

    #[test]
    fn sin_textos_de_la_pantalla_no_se_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // Sin `textos-avisos.json` no llega a llamar al servidor.
        let r = rt.block_on(comprobar(dir.path(), &SecretKey::generate()));
        assert_eq!(r.unwrap(), Vec::new());
        guardar_textos(dir.path(), &textos()).unwrap();
        assert!(dir.path().join("amigos/textos-avisos.json").exists());
    }
}
