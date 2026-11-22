/**
 * La lógica pura de «Identificar por foto»: la bandeja de cartas reconocidas
 * (la del diálogo y la del escáner en vivo). Sin React ni Tauri, con tests.
 */
import type { CardSummary } from "@/bindings/CardSummary";
import type { Reconocida } from "@/bindings/Reconocida";

/** Una foto en la bandeja. */
export interface ItemReconocido {
	clave: string;
	/** `leyendo`: aún en Rust; `fallo`: no se pudo leer la imagen. */
	estado: "leyendo" | "listo" | "fallo";
	/** El nombre del fichero (galería y escritorio). */
	nombre?: string;
	reconocida?: Reconocida;
	/** La carta (id de Scryfall) que se usará: la propuesta o la que se elija. */
	elegida?: string;
	cantidad: number;
	/** El error de Rust, si falló (`{ code, params }`). */
	error?: unknown;
}

/** Las cartas que se pueden elegir para un item: sus impresiones y luego los demás nombres propuestos. */
export function opciones(item: ItemReconocido): CardSummary[] {
	const r = item.reconocida;
	if (!r) return [];
	const vistas = new Set<string>();
	const v: CardSummary[] = [];
	for (const c of [...r.impresiones.map((i) => i.carta), ...r.candidatas]) {
		if (vistas.has(c.id)) continue;
		vistas.add(c.id);
		v.push(c);
	}
	return v;
}

/** La carta elegida de un item, si hay alguna. */
export function cartaElegida(item: ItemReconocido): CardSummary | undefined {
	if (!item.elegida) return undefined;
	return opciones(item).find((c) => c.id === item.elegida);
}

/** Pone en la bandeja una foto que se empieza a leer. */
export function empezar(items: ItemReconocido[], clave: string, nombre?: string): ItemReconocido[] {
	return [...items, { clave, estado: "leyendo", nombre, cantidad: 1 }];
}

/**
 * Llega lo reconocido de una foto. Si ya hay otra con la misma carta elegida,
 * se suma a esa (la misma carta escaneada dos veces son dos copias) y esta se quita.
 */
export function llega(items: ItemReconocido[], clave: string, r: Reconocida): ItemReconocido[] {
	const elegida = r.mejor ?? undefined;
	const otra = elegida
		? items.find((i) => i.clave !== clave && i.estado === "listo" && i.elegida === elegida)
		: undefined;
	if (otra) {
		return items
			.filter((i) => i.clave !== clave)
			.map((i) => (i.clave === otra.clave ? { ...i, cantidad: i.cantidad + 1 } : i));
	}
	const nuevo: ItemReconocido = {
		clave,
		estado: "listo",
		reconocida: r,
		elegida,
		cantidad: 1,
	};
	return items.some((i) => i.clave === clave)
		? items.map((i) => (i.clave === clave ? { ...nuevo, nombre: i.nombre } : i))
		: [...items, nuevo];
}

/** La foto no se pudo leer. */
export function falla(items: ItemReconocido[], clave: string, error: unknown): ItemReconocido[] {
	return items.map((i) => (i.clave === clave ? { ...i, estado: "fallo", error } : i));
}

export function elegir(items: ItemReconocido[], clave: string, id: string): ItemReconocido[] {
	return items.map((i) => (i.clave === clave ? { ...i, elegida: id } : i));
}

export function cambiarCantidad(
	items: ItemReconocido[],
	clave: string,
	cantidad: number,
): ItemReconocido[] {
	const n = Math.max(1, Math.min(99, Math.round(cantidad)));
	return items.map((i) => (i.clave === clave ? { ...i, cantidad: n } : i));
}

export function quitar(items: ItemReconocido[], clave: string): ItemReconocido[] {
	return items.filter((i) => i.clave !== clave);
}

/** Lo que se puede añadir: cada carta elegida con su cantidad, en orden. */
export function paraAnadir(items: ItemReconocido[]): { carta: CardSummary; cantidad: number }[] {
	return items.flatMap((i) => {
		if (i.estado !== "listo") return [];
		const carta = cartaElegida(i);
		return carta ? [{ carta, cantidad: i.cantidad }] : [];
	});
}

/** Cuántas copias hay listas para añadir. */
export function total(items: ItemReconocido[]): number {
	return paraAnadir(items).reduce((n, x) => n + x.cantidad, 0);
}

/** ¿Queda alguna foto leyéndose? */
export function leyendo(items: ItemReconocido[]): boolean {
	return items.some((i) => i.estado === "leyendo");
}
