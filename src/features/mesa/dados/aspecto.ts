import { useSyncExternalStore } from "react";
import { create } from "zustand";
import type { Acabado } from "@/bindings/Acabado";
import type { AspectoDados } from "@/bindings/AspectoDados";
import type { Metal } from "@/bindings/Metal";
import type { VistaAspectoDados } from "@/bindings/VistaAspectoDados";
import { call } from "@/lib/ipc";
import { useMesa } from "@/stores/mesa-store";
import { useRed } from "@/stores/red-store";

/**
 * **El aspecto de los dados y la moneda** de quien juega (Ajustes > Mesa).
 * Lo guarda Rust (`mesa/dados.rs`, con la imagen propia en el almacén de
 * blobs); aquí se lee una vez y se convierte en el `Estilo` que pintan los
 * tres niveles. Solo es pintura: el resultado lo decide el motor.
 */

export const ASPECTO_DE_FABRICA: AspectoDados = {
	color: null,
	numeros: "#ffffff",
	acabado: "liso",
	imagen: null,
	moneda: "oro",
};

type Estado = {
	vista: VistaAspectoDados | null;
	cargar: () => Promise<void>;
	poner: (a: AspectoDados) => Promise<void>;
	/** Una imagen del equipo (o ninguna, con `null`). */
	ponerImagen: (ruta: string | null) => Promise<void>;
};

let cargando: Promise<void> | null = null;

export const useAspectoDados = create<Estado>((set) => ({
	vista: null,
	cargar: () => {
		cargando ??= call("mesa_dados_get", {})
			.then((vista) => set({ vista }))
			.catch(() => {
				cargando = null;
			});
		return cargando;
	},
	poner: async (aspecto) => set({ vista: await call("mesa_dados_set", { aspecto }) }),
	ponerImagen: async (ruta) => set({ vista: await call("mesa_dados_imagen", { ruta }) }),
}));

/** Lo que necesita cada nivel para pintar una tirada. */
export type Estilo = {
	color: string;
	numeros: string;
	/** El trazo que rodea los números para que se lean sobre cualquier fondo. */
	contorno: string;
	acabado: Acabado;
	/** La imagen propia (`data:` URL), que tapa el color y el acabado. */
	imagen: string | null;
	moneda: Metal;
};

/** Claro u oscuro (luminancia relativa aproximada de un `#rrggbb`). */
export function esClaro(hex: string): boolean {
	const n = Number.parseInt(hex.slice(1, 7), 16);
	if (Number.isNaN(n)) return true;
	const r = (n >> 16) & 255;
	const g = (n >> 8) & 255;
	const b = n & 255;
	return 0.2126 * r + 0.7152 * g + 0.0722 * b > 140;
}

/** Si no se puede leer el color del tema (sin lienzo), este violeta. */
const COLOR_DE_RESERVA = "#7c3aed";

/** El `--primary` del tema de ahora como `#rrggbb` (a través de un lienzo: vale `oklch(…)`). */
function leerColorDeLaApp(): string {
	if (typeof document === "undefined") return COLOR_DE_RESERVA;
	const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
	if (!ctx) return COLOR_DE_RESERVA;
	const sonda = document.createElement("span");
	sonda.style.display = "none";
	sonda.style.color = "var(--primary)";
	document.body.append(sonda);
	const css = getComputedStyle(sonda).color;
	sonda.remove();
	ctx.fillStyle = "#010203";
	ctx.fillStyle = css;
	if (ctx.fillStyle === "#010203") return COLOR_DE_RESERVA;
	ctx.fillRect(0, 0, 1, 1);
	const [r = 0, g = 0, b = 0, a = 0] = ctx.getImageData(0, 0, 1, 1).data;
	if (!a) return COLOR_DE_RESERVA;
	return `#${[r, g, b].map((n) => n.toString(16).padStart(2, "0")).join("")}`;
}

let colorDeLaApp: string | null = null;

/** El tema se cambia en `<html>` (`data-theme`, la clase `dark`, variables en `style`). */
function seguirElTema(aviso: () => void): () => void {
	const o = new MutationObserver(() => {
		colorDeLaApp = null;
		aviso();
	});
	o.observe(document.documentElement, {
		attributes: true,
		attributeFilter: ["class", "data-theme", "style"],
	});
	return () => o.disconnect();
}

/**
 * **Tu color** para los dados con «Del color del jugador»: el de la app (el
 * primario del tema). Es el que enseña la muestra de Ajustes y el de tus
 * tiradas en solitario y en red, sea cual sea tu asiento.
 */
export function useColorDeLaApp(): string {
	return useSyncExternalStore(seguirElTema, () => {
		colorDeLaApp ??= leerColorDeLaApp();
		return colorDeLaApp;
	});
}

/**
 * Con qué color y si con mi aspecto se pintan las piezas que tiró `actor`.
 * Las mías llevan lo de Ajustes (y, «del color del jugador», el de la app)
 * en solitario y en red; en una partida local de varios no, porque todos
 * juegan en esta pantalla: cada uno tira con el aspecto de fábrica y el color
 * de su asiento, para que se vea quién tiró.
 */
export function useQuienTira(actor: number | null): { color: string; mia: boolean } {
	const delAsiento = useMesa((s) =>
		actor != null ? (s.jugadores[actor]?.color ?? COLOR_DE_RESERVA) : COLOR_DE_RESERVA,
	);
	const yo = useMesa((s) => s.yo);
	const varios = useMesa((s) => s.asientos.length > 1);
	const sinRed = useRed((s) => s.papel === null);
	const app = useColorDeLaApp();
	const mia = !(varios && sinRed) && actor !== null && actor === yo;
	return { color: mia ? app : delAsiento, mia };
}

/**
 * El estilo de una tirada: con el aspecto de quien juega si es suya
 * (`useQuienTira`), y si no, el de fábrica con el color de quien tiró.
 * (En línea, cada jugador mandará el suyo.)
 */
export function estiloDe(
	vista: VistaAspectoDados | null,
	colorJugador: string,
	esMia: boolean,
): Estilo {
	const a = esMia && vista ? vista.aspecto : ASPECTO_DE_FABRICA;
	return {
		color: a.color ?? colorJugador,
		numeros: a.numeros,
		contorno: esClaro(a.numeros) ? "rgba(0,0,0,.55)" : "rgba(255,255,255,.6)",
		acabado: a.acabado,
		imagen: esMia && vista ? vista.imagen : null,
		moneda: a.moneda,
	};
}

/** La clave de un estilo (para no repintar su fondo). */
export function claveEstilo(e: Estilo): string {
	return `${e.color}|${e.acabado}|${e.imagen ? e.imagen.length + e.imagen.slice(-32) : ""}`;
}

/** Los colores de cada metal de la moneda. */
export const METALES: Record<
	Metal,
	{ claro: string; oscuro: string; borde: string; canto: string; simbolo: string; trazo: string }
> = {
	oro: {
		claro: "#f8d77a",
		oscuro: "#b8862b",
		borde: "#9a6f1f",
		canto: "#c9992f",
		simbolo: "#ffffff",
		trazo: "rgba(80,50,0,.55)",
	},
	plata: {
		claro: "#f4f6f8",
		oscuro: "#99a2ac",
		borde: "#7b838c",
		canto: "#b9c0c8",
		simbolo: "#5d656e",
		trazo: "rgba(255,255,255,.7)",
	},
	cobre: {
		claro: "#f2b48a",
		oscuro: "#a4532c",
		borde: "#87411f",
		canto: "#b8653a",
		simbolo: "#fff1e6",
		trazo: "rgba(70,25,0,.55)",
	},
};
