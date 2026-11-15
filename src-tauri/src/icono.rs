//! **El icono de la app en el color del tema.** La pantalla manda los cuatro
//! colores del `<Logo>` (el primario del tema, oscurecido en degradado, y la
//! carta casi blanca) con `app_icono` cada vez que cambia el tema; aquí se
//! pinta el logo con ellos (`proxy_core::icono`) y:
//!
//! - se pone de icono de cada ventana (la barra de tareas en Windows y en los
//!   escritorios Linux que usan el de la ventana), y a los 6 s otra vez
//!   (`reavisar`), para los paneles que pintan el del tema de iconos;
//! - en Linux, además, se deja en el tema de iconos del usuario
//!   (`~/.local/share/icons/hicolor/<lado>/apps/proxy-builder.png`), que manda
//!   sobre el del sistema: así cambian también el menú, el escritorio y los
//!   paneles que usan el icono del `.desktop` (Cinnamon); y los accesos
//!   directos del escritorio apuntan a un PNG del color. Con los colores de
//!   fábrica se borra todo y vuelve el del paquete;
//! - se guardan los colores (`app_settings` `app.icono`) y al arrancar se
//!   vuelven a poner antes de enseñar la ventana: la app abre ya con su color.
//!
//! El nombre `proxy-builder` es el del icono que instala el paquete (`Icon=`
//! del `.desktop`): no se renombra nunca (invariantes del cambio de nombre).

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use ts_rs::TS;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::state::AppState;
use crate::store::settings;

const CLAVE: &str = "app.icono";

/// Los colores del logo en `#rrggbb`, como los manda la pantalla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ColoresIconoHex {
    pub claro: String,
    pub medio: String,
    pub oscuro: String,
    pub figura: String,
}

impl ColoresIconoHex {
    fn colores(&self) -> Option<proxy_core::icono::ColoresIcono> {
        use proxy_core::icono::de_hex;
        Some(proxy_core::icono::ColoresIcono {
            claro: de_hex(&self.claro)?,
            medio: de_hex(&self.medio)?,
            oscuro: de_hex(&self.oscuro)?,
            figura: de_hex(&self.figura)?,
        })
    }
}

/// Cada cambio de color sube este número: el segundo aviso al panel
/// (`reavisar`) solo va si no ha llegado otro color después.
static GENERACION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Al arrancar: el icono con los colores guardados (si los hay).
pub fn al_arrancar(app: &AppHandle, estado: &AppState) {
    let guardados = settings::get(&estado.db, CLAVE)
        .ok()
        .flatten()
        .and_then(|j| serde_json::from_str::<ColoresIconoHex>(&j).ok())
        .and_then(|c| c.colores());
    #[cfg(target_os = "linux")]
    {
        // Lo del sistema, al día, sin frenar el arranque (y la copia del
        // lanzador que dejaban versiones de prueba, fuera).
        let app = app.clone();
        let datos = estado.paths.data.clone();
        let c = guardados.unwrap_or(proxy_core::icono::DE_FABRICA);
        std::thread::spawn(move || tema_linux(&app, &datos, &c));
    }
    if let Some(c) = guardados {
        en_ventanas(app, &c, 256);
    }
}

/// Pone el icono con `hex`, lo guarda y (en Linux) lo deja en el tema de iconos.
pub fn aplicar(app: &AppHandle, estado: &AppState, hex: &ColoresIconoHex) -> AppResult<()> {
    let c = hex
        .colores()
        .ok_or_else(|| AppError::new(ErrorCode::InvalidInput).with("motivo", "color"))?;
    let json = serde_json::to_string(hex)
        .map_err(|e| AppError::new(ErrorCode::Internal).with("detalle", e))?;
    if settings::get(&estado.db, CLAVE)?.as_deref() != Some(json.as_str()) {
        settings::set(&estado.db, CLAVE, &json)?;
    }
    // Primero el tema de iconos y luego la ventana: al cambiar el icono de la
    // ventana el panel vuelve a buscar el del lanzador en el tema.
    #[cfg(target_os = "linux")]
    tema_linux(app, &estado.paths.data, &c);
    en_ventanas(app, &c, 256);
    reavisar(app, c);
    Ok(())
}

/// **Los avisos de después al panel.** El de Cinnamon (`grouped-window-list`)
/// pinta el icono del lanzador cada vez que cambia el de la ventana
/// (`notify::icon`), pero recarga el `.desktop` un momento después de que
/// cambie, y GTK solo vuelve a leer el tema si han pasado 5 s: el primer aviso
/// puede pintar el viejo. A los 2 y a los 6 s se vuelve a poner el icono de la
/// ventana (a otra medida, para que sea otro y avise) y el panel lo pinta ya
/// con el lanzador recargado. Si entretanto llegó otro color, no se hace.
fn reavisar(app: &AppHandle, c: proxy_core::icono::ColoresIcono) {
    use std::sync::atomic::Ordering;
    let g = GENERACION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        for (espera, lado) in [(2, 255), (4, 254)] {
            std::thread::sleep(std::time::Duration::from_secs(espera));
            if GENERACION.load(Ordering::SeqCst) != g {
                return;
            }
            en_ventanas(&app, &c, lado);
        }
    });
}

/// El icono de cada ventana, de `lado` px (en el móvil no hay).
#[allow(unused_variables)]
fn en_ventanas(app: &AppHandle, c: &proxy_core::icono::ColoresIcono, lado: u32) {
    #[cfg(desktop)]
    {
        use tauri::Manager;
        let Some(img) = proxy_core::icono::pintar(c, lado) else {
            return;
        };
        let (w, h) = img.dimensions();
        let rgba = img.into_raw();
        for v in app.webview_windows().values() {
            if let Err(e) = v.set_icon(tauri::image::Image::new_owned(rgba.clone(), w, h)) {
                tracing::warn!("no se pudo poner el icono de la ventana: {e}");
            }
        }
    }
}

/// Los lados que se dejan en el tema de iconos (los mismos que instala el paquete y algunos más).
#[cfg(target_os = "linux")]
const LADOS: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

/// El tema de iconos del usuario y los accesos directos del escritorio.
#[cfg(target_os = "linux")]
fn tema_linux(app: &AppHandle, datos_app: &std::path::Path, c: &proxy_core::icono::ColoresIcono) {
    use tauri::Manager;
    let Ok(datos) = app.path().data_dir() else {
        return;
    };
    let dir_png = datos_app.join("icono");
    let png = png_del_color(&dir_png, c);
    if let Ok(escritorio) = app.path().desktop_dir() {
        accesos_del_escritorio(&escritorio, png.as_deref(), &dir_png);
    }
    lanzador(&datos, png.as_deref());
    // Los PNG de colores viejos, pero no los últimos: el panel o el gestor de
    // archivos pueden estar pintando aún uno de ellos (si se borra, sale un hueco).
    pngs_viejos(&dir_png, 8);
    let raiz = datos.join("icons").join("hicolor");
    let fabrica = *c == proxy_core::icono::DE_FABRICA;
    let mut cambio = false;
    for lado in LADOS {
        let ruta = raiz
            .join(format!("{lado}x{lado}"))
            .join("apps")
            .join("proxy-builder.png");
        if fabrica {
            // El de fábrica es el del paquete: se quita el nuestro.
            cambio |= std::fs::remove_file(&ruta).is_ok();
            continue;
        }
        let Some(img) = proxy_core::icono::pintar(c, lado) else {
            continue;
        };
        let mut png = Vec::new();
        if img
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_err()
        {
            continue;
        }
        if std::fs::read(&ruta).ok().as_deref() == Some(png.as_slice()) {
            continue;
        }
        if let Some(dir) = ruta.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match crate::store::blobs::escribir_atomico(&ruta, &png) {
            Ok(()) => cambio = true,
            Err(e) => tracing::warn!(codigo = ?e.code, "no se pudo dejar el icono en el tema"),
        }
    }
    // GTK vuelve a leer el tema cuando cambia la fecha de su carpeta; también
    // deja sin valor una `icon-theme.cache` vieja.
    if cambio {
        let _ =
            std::fs::File::open(&raiz).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    }
}

/// La marca de nuestra copia del lanzador: solo se toca o se borra la que la lleva.
#[cfg(target_os = "linux")]
const MARCA_LANZADOR: &str = "# Tapete: el icono con el color del tema (src-tauri/src/icono.rs).";

/// El PNG del color en `dir_png` (un nombre por color: una ruta nueva que el
/// panel y el gestor de archivos no tienen en caché), marcado como el más
/// nuevo. `None` con los colores de fábrica (vuelve el icono del paquete).
#[cfg(target_os = "linux")]
fn png_del_color(
    dir_png: &std::path::Path,
    c: &proxy_core::icono::ColoresIcono,
) -> Option<std::path::PathBuf> {
    if *c == proxy_core::icono::DE_FABRICA {
        return None;
    }
    let nombre: String = [c.claro, c.medio, c.oscuro, c.figura]
        .iter()
        .flatten()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        + ".png";
    let png = dir_png.join(nombre);
    let _ = std::fs::create_dir_all(dir_png);
    let listo = png.is_file()
        || proxy_core::icono::pintar(c, 256)
            .is_some_and(|img| img.save_with_format(&png, image::ImageFormat::Png).is_ok());
    if !listo {
        return None;
    }
    let _ = std::fs::File::open(&png).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    Some(png)
}

/// Deja en `dir_png` solo los `deja` PNG más nuevos.
#[cfg(target_os = "linux")]
fn pngs_viejos(dir_png: &std::path::Path, deja: usize) {
    let Ok(rd) = std::fs::read_dir(dir_png) else {
        return;
    };
    let mut v: Vec<(std::time::SystemTime, std::path::PathBuf)> = rd
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (_, p) in v.into_iter().skip(deja) {
        let _ = std::fs::remove_file(p);
    }
}

/// **El lanzador.** El menú y la barra de Cinnamon pintan el icono del
/// `.desktop`, y vigilan las carpetas de lanzadores: una copia del `.desktop`
/// del sistema en `~/.local/share/applications` (que manda sobre él) con
/// `Icon=` al PNG del color hace que recarguen la app con su icono nuevo.
/// Lleva `TryExec`, así que sin la app instalada el menú la esconde; se rehace
/// desde la del sistema en cada arranque; con los colores de fábrica (`None`)
/// se borra (si es nuestra) y vuelve la del paquete. Sin `.desktop` del
/// sistema (AppImage, pruebas sin instalar) no se hace nada.
#[cfg(target_os = "linux")]
fn lanzador(datos: &std::path::Path, png: Option<&std::path::Path>) {
    const NOMBRE: &str = "Tapete.desktop";
    let copia = datos.join("applications").join(NOMBRE);
    let es_nuestra = std::fs::read_to_string(&copia).is_ok_and(|t| t.contains(MARCA_LANZADOR));
    let Some(png) = png else {
        if es_nuestra {
            let _ = std::fs::remove_file(&copia);
        }
        return;
    };
    // Una copia ajena (la escribió la persona) no se toca.
    if copia.exists() && !es_nuestra {
        return;
    }
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let Some(original) = dirs
        .split(':')
        .map(|d| std::path::Path::new(d).join("applications").join(NOMBRE))
        .find(|p| p.is_file() && *p != copia)
        .and_then(|p| std::fs::read_to_string(p).ok())
    else {
        return;
    };
    let texto = texto_lanzador(&original, png);
    if std::fs::read_to_string(&copia).ok().as_deref() != Some(texto.as_str()) {
        if let Some(dir) = copia.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = crate::store::blobs::escribir_atomico(&copia, texto.as_bytes()) {
            tracing::warn!(codigo = ?e.code, "no se pudo escribir el lanzador con el icono");
        }
    }
}

/// La copia del `.desktop` del sistema con `Icon=` en `png`, la marca y
/// `TryExec` (si no lo traía, el programa de `Exec`).
#[cfg(target_os = "linux")]
fn texto_lanzador(original: &str, png: &std::path::Path) -> String {
    let mut texto = String::new();
    let mut try_exec = false;
    for linea in original.lines() {
        if linea.starts_with("Icon=") {
            texto.push_str(&format!("Icon={}\n", png.display()));
            continue;
        }
        try_exec |= linea.starts_with("TryExec=");
        texto.push_str(linea);
        texto.push('\n');
        if linea.trim() == "[Desktop Entry]" {
            texto.push_str(MARCA_LANZADOR);
            texto.push('\n');
        }
    }
    if !try_exec {
        if let Some(exec) = original
            .lines()
            .find_map(|l| l.strip_prefix("Exec="))
            .and_then(|e| e.split_whitespace().next())
        {
            texto = texto.replacen(
                "[Desktop Entry]\n",
                &format!("[Desktop Entry]\nTryExec={exec}\n"),
                1,
            );
        }
    }
    texto
}

/// **Los accesos directos del escritorio** (copias del `.desktop` que la
/// persona dejó allí): el gestor de archivos busca su `Icon=proxy-builder` en
/// el tema de iconos solo de vez en cuando; con `Icon=` apuntando al PNG del
/// color lo repinta al momento (vigila la carpeta). Solo los que abren esta
/// app (`Exec=proxy-builder`) y cuyo icono es el del paquete o uno nuestro;
/// solo se cambia esa línea, escribiendo sobre el mismo fichero (conserva el
/// permiso de ejecución, la posición y la confianza). Con `None` (los colores
/// de fábrica), vuelve a `Icon=proxy-builder`.
#[cfg(target_os = "linux")]
fn accesos_del_escritorio(
    escritorio: &std::path::Path,
    png: Option<&std::path::Path>,
    dir_png: &std::path::Path,
) {
    let Ok(rd) = std::fs::read_dir(escritorio) else {
        return;
    };
    for e in rd.flatten() {
        let ruta = e.path();
        if ruta.extension().is_none_or(|x| x != "desktop")
            || std::fs::symlink_metadata(&ruta).map_or(true, |m| !m.is_file())
        {
            continue;
        }
        let Ok(texto) = std::fs::read_to_string(&ruta) else {
            continue;
        };
        if let Some(nuevo) = acceso_con_icono(&texto, png, dir_png) {
            if let Err(e) = std::fs::write(&ruta, nuevo) {
                tracing::warn!("no se pudo cambiar el icono del acceso directo: {e}");
            }
        }
    }
}

/// El texto de un acceso directo con su icono cambiado, o `None` si no es de
/// esta app, su icono es otro o ya está bien.
#[cfg(target_os = "linux")]
fn acceso_con_icono(
    texto: &str,
    png: Option<&std::path::Path>,
    dir_png: &std::path::Path,
) -> Option<String> {
    let abre_la_app = texto.lines().any(|l| {
        l.strip_prefix("Exec=")
            .and_then(|e| e.split_whitespace().next())
            .and_then(|p| std::path::Path::new(p).file_name())
            .is_some_and(|n| n == "proxy-builder")
    });
    if !abre_la_app {
        return None;
    }
    let icono = png.map_or_else(|| "proxy-builder".to_string(), |p| p.display().to_string());
    let mut cambio = false;
    let mut salida = String::with_capacity(texto.len() + 64);
    for linea in texto.split_inclusive('\n') {
        if let Some(v) = linea.trim_end().strip_prefix("Icon=") {
            let nuestro = v == "proxy-builder" || std::path::Path::new(v).starts_with(dir_png);
            if nuestro && v != icono {
                salida.push_str(&format!("Icon={icono}"));
                salida.push_str(&linea[linea.trim_end().len()..]);
                cambio = true;
                continue;
            }
        }
        salida.push_str(linea);
    }
    cambio.then_some(salida)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn el_lanzador_cambia_solo_el_icono() {
        let original = "[Desktop Entry]\nName=Tapete\nExec=proxy-builder %u\nIcon=proxy-builder\nType=Application\n";
        let t = texto_lanzador(original, std::path::Path::new("/d/icono/abc.png"));
        assert_eq!(
            t,
            format!(
                "[Desktop Entry]\nTryExec=proxy-builder\n{MARCA_LANZADOR}\nName=Tapete\nExec=proxy-builder %u\nIcon=/d/icono/abc.png\nType=Application\n"
            )
        );
        let con = original.replace("Name=", "TryExec=/opt/x\nName=");
        let t = texto_lanzador(&con, std::path::Path::new("/p.png"));
        assert_eq!(t.matches("TryExec=").count(), 1);
    }

    #[test]
    fn el_acceso_del_escritorio_cambia_solo_su_icono() {
        let dir = std::path::Path::new("/d/icono");
        let acceso = "[Desktop Entry]\nExec=proxy-builder\nIcon=proxy-builder\nName=Tapete\n";
        let png = std::path::Path::new("/d/icono/abc.png");
        let nuevo = acceso_con_icono(acceso, Some(png), dir).unwrap();
        assert_eq!(
            nuevo,
            acceso.replace("Icon=proxy-builder", "Icon=/d/icono/abc.png")
        );
        // Otro color, y de vuelta al de fábrica.
        let otro =
            acceso_con_icono(&nuevo, Some(std::path::Path::new("/d/icono/def.png")), dir).unwrap();
        assert!(otro.contains("Icon=/d/icono/def.png"));
        assert_eq!(acceso_con_icono(&otro, None, dir).unwrap(), acceso);
        // Ya bien, de otra app o con un icono que puso la persona: no se toca.
        assert_eq!(acceso_con_icono(acceso, None, dir), None);
        assert_eq!(
            acceso_con_icono(
                &acceso.replace("Exec=proxy-builder", "Exec=code"),
                Some(png),
                dir
            ),
            None
        );
        assert_eq!(
            acceso_con_icono(
                &acceso.replace("Icon=proxy-builder", "Icon=/mio.png"),
                Some(png),
                dir
            ),
            None
        );
    }
}
