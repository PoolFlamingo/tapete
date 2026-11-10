//! **Mensajes cifrados de extremo a extremo** entre amigos (plan Tapete, Fase D2). El servidor
//! guarda y reparte bytes que no puede leer.
//!
//! - **La clave de cifrado** es una X25519 que se **deriva** de la semilla ed25519 de la
//!   identidad (HKDF-SHA256, sal vacía, `info = "tapete/cifrado/v1"`): va siempre con la
//!   identidad (exportar e importar la identidad la lleva) y en Android queda bajo el mismo
//!   sellado del Keystore. Se publica firmada con la ed25519 (`TAPETE-v1-cifrado\n<hex>`) y la
//!   del amigo se comprueba (`verify_strict`) antes de usarla.
//! - **Cada mensaje**: `crypto_box` (X25519 + XSalsa20-Poly1305, el de libsodium) entre las dos
//!   claves, con un nonce aleatorio. En el cable: `[versión 1][nonce de 24][caja]`. Dentro, un
//!   JSON pequeño ([`Carta`]): texto o invitación a una partida, y la hora.
//!
//! Límites, dichos tal cual en la documentación: no hay secreto hacia delante (quien robe la
//! identidad de un equipo puede leer los mensajes viejos que haya guardado), y el servidor sabe
//! quién habla con quién y cuándo, aunque no qué se dicen.

use crypto_box::aead::Aead;
use crypto_box::{Nonce, PublicKey as Pub, SalsaBox, SecretKey as Sec};
use hkdf::Hkdf;
use iroh::{PublicKey, SecretKey, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::firma::hex;
use crate::error::{AppError, AppResult, ErrorCode};

pub const PREFIJO_CLAVE: &str = "TAPETE-v1-cifrado";
const INFO: &[u8] = b"tapete/cifrado/v1";
const VERSION: u8 = 1;
/// Lo más que admite el servidor por mensaje (`maxMessageBytes`), ya cifrado.
pub const MAX_CIFRADO: usize = 4096;
/// Lo más largo de un mensaje de texto (plan D4).
pub const MAX_TEXTO: usize = 2000;

/// La parte secreta de la clave de cifrado, derivada de la identidad.
pub fn secreto(identidad: &SecretKey) -> Sec {
    let hk = Hkdf::<Sha256>::new(Some(&[]), &identidad.to_bytes());
    let mut okm = [0u8; 32];
    // 32 bytes caben siempre en HKDF-SHA256 (hasta 255 × 32).
    let _ = hk.expand(INFO, &mut okm);
    Sec::from(okm)
}

/// La clave pública de cifrado (lo que se publica en `hello`).
pub fn clave_publica(identidad: &SecretKey) -> [u8; 32] {
    *secreto(identidad).public_key().as_bytes()
}

fn mensaje_de_clave(clave_cifrado_hex: &str) -> String {
    format!("{PREFIJO_CLAVE}\n{clave_cifrado_hex}")
}

/// La clave de cifrado en hex y su firma con la ed25519 de la identidad.
pub fn clave_firmada(identidad: &SecretKey) -> (String, String) {
    let clave = hex(&clave_publica(identidad));
    let firma = hex(&identidad
        .sign(mensaje_de_clave(&clave).as_bytes())
        .to_bytes());
    (clave, firma)
}

fn de_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != 2 * N || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// La clave ed25519 (identidad) de un amigo, de su hex.
pub fn identidad_de(clave_hex: &str) -> Option<PublicKey> {
    PublicKey::from_bytes(&de_hex::<32>(clave_hex)?).ok()
}

/// La clave de cifrado de un amigo, solo si la firmó su identidad (`verify_strict`).
pub fn clave_de_amigo(identidad_hex: &str, clave_hex: &str, firma_hex: &str) -> Option<Pub> {
    let identidad = identidad_de(identidad_hex)?;
    let firma = Signature::from_bytes(&de_hex::<64>(firma_hex)?);
    identidad
        .verify(mensaje_de_clave(clave_hex).as_bytes(), &firma)
        .ok()?;
    Some(Pub::from(de_hex::<32>(clave_hex)?))
}

/// Lo que va dentro de un mensaje.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Contenido {
    Texto {
        texto: String,
    },
    /// Una invitación a una partida: el código directo que lleva solo el relé (`codigoMensaje`),
    /// nunca el del QR con las IP.
    Invitacion {
        codigo: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carta {
    #[serde(flatten)]
    pub contenido: Contenido,
    /// Cuándo se escribió (ms desde 1970, el reloj de quien lo manda).
    pub enviado: i64,
}

fn invalido(motivo: &str) -> AppError {
    AppError::new(ErrorCode::InvalidInput).with("motivo", motivo)
}

/// Cifra una carta para un amigo.
pub fn cerrar(yo: &SecretKey, para: &Pub, carta: &Carta) -> AppResult<Vec<u8>> {
    if let Contenido::Texto { texto } = &carta.contenido {
        if texto.chars().count() > MAX_TEXTO {
            return Err(invalido("mensaje_largo"));
        }
    }
    let claro = serde_json::to_vec(carta).map_err(|e| invalido(&e.to_string()))?;
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce)
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))?;
    let caja = SalsaBox::new(para, &secreto(yo))
        .encrypt(Nonce::from_slice(&nonce), claro.as_slice())
        .map_err(|_| AppError::new(ErrorCode::Internal).with("detalle", "cifrar"))?;
    let mut out = Vec::with_capacity(1 + 24 + caja.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&caja);
    if out.len() > MAX_CIFRADO {
        return Err(invalido("mensaje_largo"));
    }
    Ok(out)
}

/// Abre una carta de un amigo. `None` si no es para mí, se alteró o no la escribió él.
pub fn abrir(yo: &SecretKey, de: &Pub, datos: &[u8]) -> Option<Carta> {
    let (&version, resto) = datos.split_first()?;
    if version != VERSION || resto.len() < 24 {
        return None;
    }
    let (nonce, caja) = resto.split_at(24);
    let claro = SalsaBox::new(de, &secreto(yo))
        .decrypt(Nonce::from_slice(nonce), caja)
        .ok()?;
    serde_json::from_slice(&claro).ok()
}

/// El «número de seguridad» de una amistad: 12 cifras sacadas de las dos identidades y sus
/// claves de cifrado, iguales en los dos equipos. Si alguien (un servidor malicioso) cambiara
/// una clave de cifrado, el número cambiaría: compararlo en persona o con el QR lo descubre.
pub fn numero_de_seguridad(a: (&str, &str), b: (&str, &str)) -> String {
    let (primero, segundo) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    let h = Sha256::digest(
        format!(
            "tapete/seguridad/v1\n{}\n{}\n{}\n{}",
            primero.0, primero.1, segundo.0, segundo.1
        )
        .as_bytes(),
    );
    let n = h
        .iter()
        .take(8)
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
        % 1_000_000_000_000;
    let s = format!("{n:012}");
    format!("{} {} {}", &s[0..4], &s[4..8], &s[8..12])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amigos::firma::tests::de_hex as hexa;
    use serde_json::Value;

    fn carta(t: &str) -> Carta {
        Carta {
            contenido: Contenido::Texto { texto: t.into() },
            enviado: 1_794_240_000_000,
        }
    }

    /// Los vectores compartidos con el servidor: la clave derivada y su firma.
    #[test]
    fn los_vectores_de_la_clave_de_cifrado_casan() {
        let v: Value =
            serde_json::from_str(include_str!("vectores/clave-cifrado-v1.json")).unwrap();
        assert_eq!(v["formato"], "tapete-clave-cifrado-v1");
        assert_eq!(v["prefijo"], PREFIJO_CLAVE);
        assert_eq!(v["derivacion"]["info"], "tapete/cifrado/v1");
        for caso in v["casos"].as_array().unwrap() {
            let nombre = caso["nombre"].as_str().unwrap();
            let semilla: [u8; 32] = hexa(caso["semilla"].as_str().unwrap());
            let id = SecretKey::from_bytes(&semilla);
            let clave_hex = caso["claveCifrado"].as_str().unwrap();
            let firma_hex = caso["firma"].as_str().unwrap();
            let ok = clave_de_amigo(caso["clavePublica"].as_str().unwrap(), clave_hex, firma_hex)
                .is_some();
            assert_eq!(ok, caso["valida"].as_bool().unwrap(), "{nombre}");
            if caso["valida"].as_bool().unwrap() && nombre.starts_with("derivada") {
                assert_eq!(
                    hex(&secreto(&id).to_bytes()),
                    caso["secretoCifrado"].as_str().unwrap(),
                    "{nombre}"
                );
                let (c, f) = clave_firmada(&id);
                assert_eq!(c, clave_hex, "{nombre}");
                assert_eq!(f, firma_hex, "{nombre}");
                assert_eq!(caso["mensaje"].as_str().unwrap(), mensaje_de_clave(&c));
            }
        }
    }

    #[test]
    fn la_misma_identidad_da_la_misma_clave() {
        let a = SecretKey::from_bytes(&[3; 32]);
        assert_eq!(
            clave_publica(&a),
            clave_publica(&SecretKey::from_bytes(&[3; 32]))
        );
        assert_ne!(
            clave_publica(&a),
            clave_publica(&SecretKey::from_bytes(&[4; 32]))
        );
    }

    #[test]
    fn ida_y_vuelta_y_lo_alterado_o_ajeno_no_se_abre() {
        let ana = SecretKey::from_bytes(&[1; 32]);
        let bea = SecretKey::from_bytes(&[2; 32]);
        let eva = SecretKey::from_bytes(&[9; 32]);
        let pub_de = |k: &SecretKey| Pub::from(clave_publica(k));
        let c = carta("hola 👋");
        let cifrado = cerrar(&ana, &pub_de(&bea), &c).unwrap();
        assert_eq!(cifrado[0], VERSION);
        assert_eq!(abrir(&bea, &pub_de(&ana), &cifrado), Some(c.clone()));
        // Dos veces el mismo texto, dos cifrados distintos (nonce nuevo).
        assert_ne!(cifrado, cerrar(&ana, &pub_de(&bea), &c).unwrap());
        // Alterado.
        let mut malo = cifrado.clone();
        *malo.last_mut().unwrap() ^= 1;
        assert_eq!(abrir(&bea, &pub_de(&ana), &malo), None);
        // Para otra, o dicho que viene de otra.
        assert_eq!(abrir(&eva, &pub_de(&ana), &cifrado), None);
        assert_eq!(abrir(&bea, &pub_de(&eva), &cifrado), None);
        // Versión desconocida.
        let mut v2 = cifrado;
        v2[0] = 2;
        assert_eq!(abrir(&bea, &pub_de(&ana), &v2), None);
    }

    #[test]
    fn una_invitacion_viaja_igual() {
        let ana = SecretKey::from_bytes(&[1; 32]);
        let bea = SecretKey::from_bytes(&[2; 32]);
        let c = Carta {
            contenido: Contenido::Invitacion {
                codigo: "PB1-abc".into(),
            },
            enviado: 5,
        };
        let x = cerrar(&ana, &Pub::from(clave_publica(&bea)), &c).unwrap();
        assert_eq!(abrir(&bea, &Pub::from(clave_publica(&ana)), &x), Some(c));
    }

    #[test]
    fn los_mensajes_largos_no_se_mandan() {
        let ana = SecretKey::from_bytes(&[1; 32]);
        let para = Pub::from(clave_publica(&SecretKey::from_bytes(&[2; 32])));
        assert!(cerrar(&ana, &para, &carta(&"a".repeat(MAX_TEXTO))).is_ok());
        assert!(cerrar(&ana, &para, &carta(&"a".repeat(MAX_TEXTO + 1))).is_err());
        // 2.000 caracteres de 4 bytes no caben en los 4 KiB del servidor: se dice, no se corta.
        assert!(cerrar(&ana, &para, &carta(&"😀".repeat(MAX_TEXTO))).is_err());
    }

    #[test]
    fn el_numero_de_seguridad_es_el_mismo_en_los_dos_lados_y_cambia_con_la_clave() {
        let a = ("aa", "11");
        let b = ("bb", "22");
        let n = numero_de_seguridad(a, b);
        assert_eq!(n, numero_de_seguridad(b, a));
        assert_eq!(n.len(), 14);
        assert_ne!(n, numero_de_seguridad(a, ("bb", "23")));
    }
}
