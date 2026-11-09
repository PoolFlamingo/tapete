//! Pinta el logo (`src-tauri/icons/icono.svg` y `icono-frente.svg`) a PNG de 1024 px para
//! `pnpm tauri icon`. Con resvg, que ya está en el árbol por `proxy-core`: así el icono no
//! depende de un programa del sistema que pinte SVG (degradados incluidos).
//!
//! `cargo run -p proxy-core --example icono` (desde `src-tauri/`).

use std::path::Path;

fn pintar(svg: &Path, png: &Path, lado: u32) -> Result<(), Box<dyn std::error::Error>> {
    let datos = std::fs::read(svg)?;
    let arbol = resvg::usvg::Tree::from_data(&datos, &resvg::usvg::Options::default())?;
    let mut lienzo = tiny_skia::Pixmap::new(lado, lado).ok_or("lienzo vacío")?;
    let escala = lado as f32 / arbol.size().width();
    resvg::render(
        &arbol,
        tiny_skia::Transform::from_scale(escala, escala),
        &mut lienzo.as_mut(),
    );
    // tiny-skia guarda el color premultiplicado por el alfa; el PNG lo quiere sin.
    let mut rgba = Vec::with_capacity((lado * lado * 4) as usize);
    for p in lienzo.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let img = image::RgbaImage::from_raw(lado, lado, rgba).ok_or("medidas")?;
    img.save(png)?;
    println!("{}", png.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let iconos = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../icons");
    pintar(
        &iconos.join("icono.svg"),
        &iconos.join("icono-1024.png"),
        1024,
    )?;
    pintar(
        &iconos.join("icono-frente.svg"),
        &iconos.join("icono-frente-1024.png"),
        1024,
    )?;
    Ok(())
}
