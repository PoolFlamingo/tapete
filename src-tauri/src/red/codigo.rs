//! **El código directo** (sin servidor): lo que el anfitrión pasa a los demás
//! para que se unan. Lleva su dirección de iroh (`EndpointAddr`: su clave y
//! por dónde llegar) y un secreto de 16 bytes que el anfitrión pide en el
//! `Hola`: sin él no admite a nadie. Es texto: `PB1-` + base64url del JSON.
//!
//! El de **pasar decks** (`pasar/`) tiene el mismo formato con otro prefijo,
//! `PBD1-`: pegar uno en el sitio del otro da un error claro (`motivo`
//! `codigo_de_deck` o `codigo_de_partida`) y no una conexión rara.

use base64::Engine;
use iroh::EndpointAddr;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, ErrorCode};

const PREFIJO: &str = "PB1-";
/// El de pasar un deck a otro equipo.
const PREFIJO_DECK: &str = "PBD1-";
/// Lo más largo que se acepta pegar.
const MAX: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Codigo {
    #[serde(rename = "a")]
    pub addr: EndpointAddr,
    /// El secreto, en hexadecimal (32 caracteres).
    #[serde(rename = "s")]
    pub secreto: String,
}

/// Un secreto nuevo de 16 bytes del generador del sistema, en hexadecimal.
/// 🛑 Sin entropía no hay secreto: da un error, nunca uno adivinable.
/// El mismo código de partida sin las IP del anfitrión: solo su clave, el relé y el secreto. Es
/// el que va dentro de una invitación a un amigo (plan Tapete, D5): la IP no se le da a nadie
/// que no haya entrado ya a la partida. Sin relé no sirve a distancia: error `sin_rele`.
pub fn solo_rele(t: &str) -> AppResult<String> {
    let mut c = Codigo::de_texto(t)?;
    c.addr
        .addrs
        .retain(|a| matches!(a, iroh::TransportAddr::Relay(_)));
    if c.addr.addrs.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput).with("motivo", "sin_rele"));
    }
    c.a_texto()
}

pub fn secreto_nuevo() -> AppResult<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b)
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e.to_string()))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Compara dos secretos sin cortar en la primera diferencia.
pub fn mismo_secreto(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

impl Codigo {
    pub fn a_texto(&self) -> AppResult<String> {
        self.a_texto_con(PREFIJO)
    }

    /// El código de pasar un deck (`PBD1-…`).
    pub fn a_texto_deck(&self) -> AppResult<String> {
        self.a_texto_con(PREFIJO_DECK)
    }

    fn a_texto_con(&self, prefijo: &str) -> AppResult<String> {
        let j = serde_json::to_vec(self)
            .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))?;
        Ok(format!(
            "{prefijo}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(j)
        ))
    }

    /// Un código de partida (`PB1-…`). Uno de pasar decks da `codigo_de_deck`.
    pub fn de_texto(t: &str) -> AppResult<Codigo> {
        Self::de_texto_con(t, PREFIJO, PREFIJO_DECK, "codigo_de_deck")
    }

    /// Un código de pasar un deck (`PBD1-…`). Uno de partida da
    /// `codigo_de_partida`.
    pub fn de_texto_deck(t: &str) -> AppResult<Codigo> {
        Self::de_texto_con(t, PREFIJO_DECK, PREFIJO, "codigo_de_partida")
    }

    fn de_texto_con(t: &str, prefijo: &str, otro: &str, motivo_otro: &str) -> AppResult<Codigo> {
        let malo = || AppError::new(ErrorCode::InvalidInput).with("motivo", "codigo");
        let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        if t.len() > MAX {
            return Err(malo());
        }
        let cuerpo = match t.strip_prefix(prefijo) {
            Some(c) => c,
            None if t.starts_with(otro) => {
                return Err(AppError::new(ErrorCode::InvalidInput).with("motivo", motivo_otro))
            }
            None => return Err(malo()),
        };
        let j = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(cuerpo)
            .map_err(|_| malo())?;
        let c: Codigo = serde_json::from_slice(&j).map_err(|_| malo())?;
        if c.secreto.len() != 32 || !c.secreto.chars().all(|x| x.is_ascii_hexdigit()) {
            return Err(malo());
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_invitacion_lleva_el_rele_y_nunca_las_ip() {
        let k = iroh::SecretKey::generate();
        let rele: iroh::RelayUrl = "https://rele.ejemplo/".parse().unwrap();
        let c = Codigo {
            addr: EndpointAddr::new(k.public()).with_addrs([
                iroh::TransportAddr::Ip("192.168.1.20:4242".parse().unwrap()),
                iroh::TransportAddr::Ip("[2001:db8::1]:4242".parse().unwrap()),
                iroh::TransportAddr::Relay(rele.clone()),
            ]),
            secreto: "ab".repeat(16),
        };
        let solo = Codigo::de_texto(&solo_rele(&c.a_texto().unwrap()).unwrap()).unwrap();
        assert_eq!(solo.addr.id, k.public());
        assert_eq!(solo.secreto, c.secreto);
        assert_eq!(solo.addr.ip_addrs().count(), 0);
        assert_eq!(solo.addr.relay_urls().collect::<Vec<_>>(), [&rele]);
        // Sin relé no vale para invitar a distancia.
        let sin = Codigo {
            addr: EndpointAddr::new(k.public())
                .with_addrs([iroh::TransportAddr::Ip("10.0.0.1:1".parse().unwrap())]),
            secreto: "ab".repeat(16),
        };
        let e = solo_rele(&sin.a_texto().unwrap()).unwrap_err();
        assert_eq!(e.params.get("motivo").map(String::as_str), Some("sin_rele"));
    }

    #[test]
    fn ida_y_vuelta_y_codigos_malos() {
        let k = iroh::SecretKey::generate();
        let c = Codigo {
            addr: EndpointAddr::new(k.public()),
            secreto: secreto_nuevo().unwrap(),
        };
        let t = c.a_texto().unwrap();
        assert!(t.starts_with("PB1-"));
        // Con espacios o saltos de línea al pegarlo, igual.
        let pegado = format!("  {}\n{} ", &t[..10], &t[10..]);
        assert_eq!(Codigo::de_texto(&pegado).unwrap(), c);
        for malo in [
            "",
            "PB1-",
            "PB2-abc",
            "PB1-%%%",
            &format!("PB1-{}", "a".repeat(5_000)),
        ] {
            assert!(Codigo::de_texto(malo).is_err(), "{malo}");
        }
    }

    #[test]
    fn secretos() {
        let a = secreto_nuevo().unwrap();
        assert_eq!(a.len(), 32);
        assert_ne!(a, secreto_nuevo().unwrap());
        assert!(mismo_secreto(&a, &a.clone()));
        assert!(!mismo_secreto(&a, &secreto_nuevo().unwrap()));
        assert!(!mismo_secreto(&a, "corto"));
    }

    #[test]
    fn el_codigo_de_un_deck_y_el_de_una_partida_no_se_confunden() {
        let k = iroh::SecretKey::generate();
        let c = Codigo {
            addr: EndpointAddr::new(k.public()),
            secreto: secreto_nuevo().unwrap(),
        };
        let deck = c.a_texto_deck().unwrap();
        let partida = c.a_texto().unwrap();
        assert!(deck.starts_with("PBD1-"));
        assert_eq!(Codigo::de_texto_deck(&deck).unwrap(), c);
        // Cada uno en su sitio.
        let e = Codigo::de_texto(&deck).unwrap_err();
        assert_eq!(e.params["motivo"], "codigo_de_deck");
        let e = Codigo::de_texto_deck(&partida).unwrap_err();
        assert_eq!(e.params["motivo"], "codigo_de_partida");
        // Y lo que no es ninguno, el error de siempre.
        assert_eq!(
            Codigo::de_texto_deck("PB2-abc").unwrap_err().params["motivo"],
            "codigo"
        );
        assert_eq!(
            Codigo::de_texto("PBD2-abc").unwrap_err().params["motivo"],
            "codigo"
        );
    }
}
