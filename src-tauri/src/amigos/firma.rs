//! **La firma de cada petición al servidor de Tapete** (contrato v1, `docs/api.md` del
//! servidor): cuatro cabeceras `X-Tapete-*` con la clave ed25519 de la identidad de la app (la
//! misma de iroh: no hay otra clave que guardar ni que sellar en Android).
//!
//! Se firma, en UTF-8 y sin `\n` final:
//!
//! ```text
//! TAPETE-v1
//! <MÉTODO>
//! <RUTA?query tal cual viaja>
//! <timestamp en segundos>
//! <nonce: 16 bytes en hex>
//! <sha256 en hex del cuerpo crudo>
//! ```
//!
//! 🛑 Los vectores de `vectores/firma-v1.json` son una **copia idéntica** de los del servidor
//! (`tests/vectores/` de `tapete-server`): si no casan, la app y el servidor no se entienden.

use iroh::SecretKey;
use sha2::{Digest, Sha256};

pub const PREFIJO: &str = "TAPETE-v1";

/// Las cabeceras que lleva una petición firmada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cabeceras {
    pub clave: String,
    pub timestamp: String,
    pub nonce: String,
    pub firma: String,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(datos: &[u8]) -> String {
    hex(&Sha256::digest(datos))
}

/// La cadena que se firma.
pub fn cadena(metodo: &str, ruta: &str, timestamp: u64, nonce: &str, cuerpo: &[u8]) -> String {
    format!(
        "{PREFIJO}\n{}\n{ruta}\n{timestamp}\n{nonce}\n{}",
        metodo.to_ascii_uppercase(),
        sha256_hex(cuerpo)
    )
}

/// Firma una petición con un nonce y una hora dados (los tests los fijan).
pub fn firmar_con(
    clave: &SecretKey,
    metodo: &str,
    ruta: &str,
    cuerpo: &[u8],
    timestamp: u64,
    nonce: [u8; 16],
) -> Cabeceras {
    let nonce = hex(&nonce);
    let c = cadena(metodo, ruta, timestamp, &nonce, cuerpo);
    Cabeceras {
        clave: hex(clave.public().as_bytes()),
        timestamp: timestamp.to_string(),
        nonce,
        firma: hex(&clave.sign(c.as_bytes()).to_bytes()),
    }
}

/// Firma una petición ahora, con un nonce nuevo.
pub fn firmar(clave: &SecretKey, metodo: &str, ruta: &str, cuerpo: &[u8]) -> Cabeceras {
    let mut nonce = [0u8; 16];
    // Sin aleatoriedad del sistema no hay nonce fiable: mejor un fallo del servidor
    // (`replayed_nonce`) que mandar siempre el mismo.
    let _ = getrandom::fill(&mut nonce);
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    firmar_con(clave, metodo, ruta, cuerpo, ahora, nonce)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::Value;

    pub(crate) fn de_hex<const N: usize>(s: &str) -> [u8; N] {
        let mut out = [0u8; N];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        }
        out
    }

    fn vectores() -> Value {
        serde_json::from_str(include_str!("vectores/firma-v1.json")).unwrap()
    }

    /// Los vectores compartidos con el servidor: la cadena y la firma de la app son, byte a
    /// byte, las que el servidor espera.
    #[test]
    fn los_vectores_del_servidor_casan() {
        let v = vectores();
        assert_eq!(v["formato"], "tapete-firma-v1");
        assert_eq!(v["prefijo"], PREFIJO);
        let mut validos = 0;
        for caso in v["casos"].as_array().unwrap() {
            let nombre = caso["nombre"].as_str().unwrap();
            let cuerpo = caso["cuerpo"].as_str().unwrap().as_bytes();
            assert_eq!(
                sha256_hex(cuerpo),
                caso["cuerpoSha256"].as_str().unwrap(),
                "{nombre}"
            );
            let ts: u64 = caso["timestamp"].as_str().unwrap().parse().unwrap();
            let c = cadena(
                caso["metodo"].as_str().unwrap(),
                caso["ruta"].as_str().unwrap(),
                ts,
                caso["nonce"].as_str().unwrap(),
                cuerpo,
            );
            if caso["valida"].as_bool().unwrap() {
                validos += 1;
                let semilla: [u8; 32] = de_hex(caso["semilla"].as_str().unwrap());
                let clave = SecretKey::from_bytes(&semilla);
                assert_eq!(
                    hex(clave.public().as_bytes()),
                    caso["clavePublica"].as_str().unwrap(),
                    "{nombre}"
                );
                assert_eq!(c, caso["cadenaCanonica"].as_str().unwrap(), "{nombre}");
                let h = firmar_con(
                    &clave,
                    caso["metodo"].as_str().unwrap(),
                    caso["ruta"].as_str().unwrap(),
                    cuerpo,
                    ts,
                    de_hex(caso["nonce"].as_str().unwrap()),
                );
                assert_eq!(h.firma, caso["firma"].as_str().unwrap(), "{nombre}");
                assert_eq!(h.nonce, caso["nonce"].as_str().unwrap(), "{nombre}");
                assert_eq!(h.timestamp, caso["timestamp"].as_str().unwrap(), "{nombre}");
            } else if caso["motivo"] == "bad_signature" || caso["motivo"] == "bad_key" {
                // Lo que el servidor rechaza tampoco lo da por bueno `verify_strict`.
                let clave_pub: [u8; 32] = de_hex(caso["clavePublica"].as_str().unwrap());
                let firma: [u8; 64] = de_hex(caso["firma"].as_str().unwrap());
                let ok = iroh::PublicKey::from_bytes(&clave_pub).is_ok_and(|k| {
                    k.verify(c.as_bytes(), &iroh::Signature::from_bytes(&firma))
                        .is_ok()
                });
                assert!(!ok, "{nombre}");
            }
        }
        assert!(validos >= 4);
    }

    #[test]
    fn cada_peticion_lleva_un_nonce_nuevo() {
        let k = SecretKey::from_bytes(&[7; 32]);
        let a = firmar(&k, "get", "/api/v1/sync", b"");
        let b = firmar(&k, "GET", "/api/v1/sync", b"");
        assert_ne!(a.nonce, b.nonce);
        assert_eq!(a.nonce.len(), 32);
        assert_eq!(a.firma.len(), 128);
        assert_eq!(a.clave.len(), 64);
    }
}
