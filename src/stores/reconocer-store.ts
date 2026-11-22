/**
 * **Identificar por foto**: qué se está identificando y para qué. Lo comparten
 * el diálogo (escritorio y galería) y el escáner en vivo de Android: la
 * bandeja es la misma, así que se puede pasar de uno a otro sin perderla.
 */
import { create } from "zustand";
import type { Reconocida } from "@/bindings/Reconocida";
import {
	cambiarCantidad,
	elegir,
	empezar,
	falla,
	type ItemReconocido,
	llega,
	quitar,
} from "@/features/reconocer/cuentas";
import { call } from "@/lib/ipc";

/** Para qué se identifica: consultar la carta (Buscar) o meterla en el deck. */
export type ModoReconocer =
	| { tipo: "buscar" }
	/** Como «Añadir carta»: `en` es la posición y `hueco` la casilla en blanco que se rellena. */
	| { tipo: "deck"; en?: number; hueco?: string };

interface EstadoReconocer {
	/** El diálogo de fotos está abierto. */
	abierto: boolean;
	/** El escáner en vivo está abierto (solo Android). */
	vivo: boolean;
	modo: ModoReconocer;
	items: ItemReconocido[];
	abrir: (modo: ModoReconocer, vivo: boolean) => void;
	/** Cierra el diálogo y el escáner y vacía la bandeja. */
	cerrar: () => void;
	/** Pasa del escáner al diálogo (o al revés) sin perder la bandeja. */
	verVivo: (vivo: boolean) => void;
	/** Lee unas fotos (rutas o URIs de Android), una detrás de otra. */
	leer: (fotos: { ruta: string; nombre?: string }[]) => Promise<void>;
	/** Lo que reconoce el escáner en vivo. */
	recibir: (r: Reconocida) => void;
	elegir: (clave: string, id: string) => void;
	cantidad: (clave: string, n: number) => void;
	quitar: (clave: string) => void;
	vaciar: () => void;
}

let siguiente = 0;
const nuevaClave = (prefijo: string) => `${prefijo}-${Date.now()}-${siguiente++}`;

/** Las fotos van de una en una: Rust ya reparte el trabajo y así la bandeja se llena en orden. */
let cola: Promise<void> = Promise.resolve();

export const useReconocer = create<EstadoReconocer>((set, get) => ({
	abierto: false,
	vivo: false,
	modo: { tipo: "buscar" },
	items: [],
	abrir: (modo, vivo) => set({ modo, abierto: !vivo, vivo, items: [] }),
	cerrar: () => set({ abierto: false, vivo: false, items: [] }),
	verVivo: (vivo) => set({ vivo, abierto: !vivo }),
	leer: (fotos) => {
		const claves = fotos.map((f) => ({ ...f, clave: nuevaClave("foto") }));
		set((s) => ({ items: claves.reduce((v, f) => empezar(v, f.clave, f.nombre), s.items) }));
		const trabajo = cola.then(async () => {
			for (const f of claves) {
				// Si se cerró mientras tanto, no se sigue leyendo.
				if (!get().items.some((i) => i.clave === f.clave)) continue;
				try {
					const r = await call("reconocer_foto", { ruta: f.ruta });
					if (get().items.some((i) => i.clave === f.clave)) {
						set((s) => ({ items: llega(s.items, f.clave, r) }));
					}
				} catch (e) {
					set((s) => ({ items: falla(s.items, f.clave, e) }));
				}
			}
		});
		cola = trabajo.catch(() => {});
		return trabajo;
	},
	recibir: (r) => set((s) => ({ items: llega(s.items, nuevaClave("vivo"), r) })),
	elegir: (clave, id) => set((s) => ({ items: elegir(s.items, clave, id) })),
	cantidad: (clave, n) => set((s) => ({ items: cambiarCantidad(s.items, clave, n) })),
	quitar: (clave) => set((s) => ({ items: quitar(s.items, clave) })),
	vaciar: () => set({ items: [] }),
}));
