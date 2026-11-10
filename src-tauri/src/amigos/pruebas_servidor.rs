//! **Prueba contra un servidor de Tapete de verdad** (plan D8). No va con el resto: necesita el
//! servidor corriendo (`pnpm dev` en `tapete-server`, puerto 1530):
//!
//! ```text
//! PB_TAPETE=http://127.0.0.1:1530 cargo test --lib amigos::pruebas_servidor -- --ignored
//! ```
//!
//! Dos identidades nuevas se presentan, se hacen amigas, se mandan un mensaje y una invitación
//! cifrados, los recogen, los confirman y ven el acuse de entrega.

use base64::Engine;
use iroh::SecretKey;

use super::cifrado::{self, Carta, Contenido};
use super::cliente::{base, Cliente, Hola};

fn identidad() -> SecretKey {
    let mut s = [0u8; 32];
    getrandom::fill(&mut s).unwrap();
    SecretKey::from_bytes(&s)
}

async fn alta(c: &Cliente, k: &SecretKey, apodo: &str) -> String {
    let (clave, firma) = cifrado::clave_firmada(k);
    c.hola(
        k,
        &Hola {
            nickname: apodo,
            platform: "linux",
            version: "4.1.0",
            encryption_key: &clave,
            encryption_key_signature: &firma,
        },
    )
    .await
    .unwrap()
    .friend_code
}

#[tokio::test]
#[ignore]
async fn dos_equipos_se_hacen_amigos_y_se_escriben_cifrado() {
    let c = Cliente::nuevo(base()).unwrap();
    let ana = identidad();
    let bea = identidad();
    let _codigo_ana = alta(&c, &ana, "Ana prueba").await;
    let codigo_bea = alta(&c, &bea, "Bea prueba").await;
    assert!(codigo_bea.starts_with("TAP-"));

    // Ana pide, Bea acepta.
    let r = c.pedir_amistad(&ana, &codigo_bea).await.unwrap();
    assert_eq!(r.status, "pending");
    let s = c.sync(&bea, None).await.unwrap();
    let solicitud = s
        .requests
        .incoming
        .first()
        .expect("la solicitud de Ana")
        .clone();
    assert_eq!(solicitud.nickname, "Ana prueba");
    c.responder(&bea, &solicitud.id, true).await.unwrap();

    // Ana ve a Bea de amiga, con su clave de cifrado firmada.
    let s = c.sync(&ana, None).await.unwrap();
    let amiga = s
        .friends
        .iter()
        .find(|f| f.nickname == "Bea prueba")
        .unwrap()
        .clone();
    let para = cifrado::clave_de_amigo(
        &amiga.key,
        &amiga.encryption_key,
        &amiga.encryption_key_signature,
    )
    .expect("la clave de Bea viene firmada por Bea");

    // Ana escribe y luego invita.
    for (i, contenido) in [
        Contenido::Texto {
            texto: "hola 👋".into(),
        },
        Contenido::Invitacion {
            codigo: "PB1-prueba".into(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let datos = cifrado::cerrar(
            &ana,
            &para,
            &Carta {
                contenido,
                enviado: 1,
            },
        )
        .unwrap();
        let id_cliente = format!("{i:032x}");
        c.mandar(&ana, &amiga.key, &datos, &id_cliente)
            .await
            .unwrap();
        // Reintentar con el mismo id no duplica.
        c.mandar(&ana, &amiga.key, &datos, &id_cliente)
            .await
            .unwrap();
    }

    // Bea los recoge, los abre y los confirma.
    let s = c.sync(&bea, None).await.unwrap();
    assert_eq!(s.messages.len(), 2, "sin duplicados");
    let amiga_ana = s
        .friends
        .iter()
        .find(|f| f.nickname == "Ana prueba")
        .unwrap();
    let de = cifrado::clave_de_amigo(
        &amiga_ana.key,
        &amiga_ana.encryption_key,
        &amiga_ana.encryption_key_signature,
    )
    .unwrap();
    let cartas: Vec<Carta> = s
        .messages
        .iter()
        .map(|m| {
            let datos = base64::engine::general_purpose::STANDARD
                .decode(&m.ciphertext)
                .unwrap();
            cifrado::abrir(&bea, &de, &datos).expect("se abre")
        })
        .collect();
    assert!(matches!(&cartas[0].contenido, Contenido::Texto { texto } if texto == "hola 👋"));
    assert!(
        matches!(&cartas[1].contenido, Contenido::Invitacion { codigo } if codigo == "PB1-prueba")
    );
    let ids: Vec<String> = s.messages.iter().map(|m| m.id.clone()).collect();
    c.confirmar(&bea, &ids).await.unwrap();
    assert!(c.sync(&bea, None).await.unwrap().messages.is_empty());

    // Ana ve los dos acuses de entrega con sus ids.
    let s = c.sync(&ana, None).await.unwrap();
    let mut entregados: Vec<String> = s
        .deliveries
        .iter()
        .filter_map(|d| d.client_id.clone())
        .collect();
    entregados.sort();
    assert_eq!(entregados, [format!("{:032x}", 0), format!("{:032x}", 1)]);

    // Limpieza: las dos se borran del servidor.
    c.borrarme(&ana).await.unwrap();
    c.borrarme(&bea).await.unwrap();
}
