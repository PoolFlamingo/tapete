//! **El TLS de cada plataforma**, en un solo sitio. Hay dos consumidores: los
//! clientes HTTP (`reqwest`: Scryfall, el actualizador, el servidor de Tapete…) y
//! el nodo de iroh.
//!
//! En escritorio, el verificador del sistema (los certificados que el sistema ya
//! tiene, con su revocación). En Android (y iOS,
//! hasta que se pruebe allí) ese verificador entra en pánico si no se inicializa
//! con su parte de Kotlin, así que en móvil se usan los certificados raíz de
//! Mozilla incrustados en la app: sin Kotlin ni JNI en el arranque. Se pierden
//! las autoridades que alguien instale a mano en el teléfono, que esta app no
//! necesita.
//!
//! 🛑 Nadie más crea un cliente: ningún `reqwest::Client::builder()`,
//! `with_platform_verifier` ni `CaTlsConfig::` fuera de este fichero (lo
//! vigila un test).

/// El `User-Agent` de todas las peticiones de la app: Scryfall pide nombre y una forma de
/// contacto, y lo ven también GitHub, Google Fonts y los demás sitios de `fuentes`.
pub const AGENTE: &str = concat!(
    "Tapete/",
    env!("CARGO_PKG_VERSION"),
    " (+https://tapete.zodhub.app)"
);

/// Un `reqwest::ClientBuilder` con el TLS de esta plataforma. Se usa así:
/// `tls::reqwest(reqwest::Client::builder()).user_agent(tls::AGENTE).build()`.
pub fn reqwest(b: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    #[cfg(mobile)]
    {
        b.tls_certs_only(raices_reqwest().iter().cloned())
    }
    #[cfg(not(mobile))]
    {
        b
    }
}

/// Los certificados raíz de Mozilla como los quiere `reqwest`, hechos una vez.
#[cfg(mobile)]
fn raices_reqwest() -> &'static [reqwest::Certificate] {
    use std::sync::OnceLock;
    static RAICES: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();
    RAICES.get_or_init(|| {
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|c| reqwest::Certificate::from_der(c.as_ref()).ok())
            .collect()
    })
}

/// Los certificados con los que el nodo de iroh comprueba los relés.
pub fn iroh() -> iroh::tls::CaTlsConfig {
    #[cfg(mobile)]
    {
        iroh::tls::CaTlsConfig::embedded()
    }
    #[cfg(not(mobile))]
    {
        iroh::tls::CaTlsConfig::system()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn ficheros(dir: &Path, fuera: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                ficheros(&p, fuera);
            } else if p.extension().is_some_and(|x| x == "rs") {
                fuera.push(p);
            }
        }
    }

    #[test]
    fn nadie_crea_un_cliente_ni_un_verificador_fuera_de_tls_rs() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut todos = Vec::new();
        ficheros(&src, &mut todos);
        let mut mal = Vec::new();
        for f in todos {
            if f.file_name().is_some_and(|n| n == "tls.rs") {
                continue;
            }
            let texto = std::fs::read_to_string(&f).unwrap();
            for (i, l) in texto.lines().enumerate() {
                let l = l.trim();
                if l.starts_with("//") {
                    continue;
                }
                let cliente = l.contains("Client::builder()") && !l.contains("tls::reqwest(");
                if cliente || l.contains("with_platform_verifier") || l.contains("CaTlsConfig::") {
                    mal.push(format!("{}:{}", f.display(), i + 1));
                }
            }
        }
        assert!(mal.is_empty(), "TLS fuera de tls.rs: {mal:?}");
    }
}
