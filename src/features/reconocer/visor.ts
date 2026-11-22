/**
 * La geometría del escáner en vivo (pura, con tests): el marco guía y dónde cae
 * en la pantalla la carta que ve Rust.
 */
import type { Punto } from "@/bindings/Punto";

/** Proporción de una carta de Magic (63 × 88 mm). */
const PROPORCION = 63 / 88;

/** El marco guía: una carta centrada que ocupa como mucho el 75 % de la caja. */
export function marcoGuia(w: number, h: number): { x: number; y: number; w: number; h: number } {
	const ancho = Math.min(w * 0.75, h * 0.75 * PROPORCION);
	const alto = ancho / PROPORCION;
	return { x: (w - ancho) / 2, y: (h - alto) / 2, w: ancho, h: alto };
}

/**
 * Las esquinas (en fracciones del fotograma de `ancho × alto`) en píxeles de
 * la caja del visor. La vista previa llena la pantalla recortando lo que sobra
 * por los lados (`FILL_CENTER` de CameraX), así que se encaja igual; `origen`
 * es dónde empieza la caja en la pantalla.
 */
export function encajar(
	esquinas: Punto[],
	ancho: number,
	alto: number,
	pantalla: { w: number; h: number },
	origen: { left: number; top: number } = { left: 0, top: 0 },
): Punto[] {
	const escala = Math.max(pantalla.w / Math.max(1, ancho), pantalla.h / Math.max(1, alto));
	const w = ancho * escala;
	const h = alto * escala;
	const x0 = (pantalla.w - w) / 2 - origen.left;
	const y0 = (pantalla.h - h) / 2 - origen.top;
	return esquinas.map((p) => ({ x: x0 + p.x * w, y: y0 + p.y * h }));
}
