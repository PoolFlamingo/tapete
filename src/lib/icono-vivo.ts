import type { ColoresIconoHex } from "@/bindings/ColoresIconoHex";
import { call } from "@/lib/ipc";

/**
 * **El icono de la app en el color del tema** (solo escritorio). Cada vez que
 * cambia el tema (`data-theme`, la clase `dark`, el modo del sistema) se leen
 * los colores del `<Logo>` (`components/logo.tsx`: el primario oscurecido al
 * 80, 55 y 35 % y la carta casi blanca) y se mandan a Rust (`app_icono`), que
 * pinta el icono de la ventana, lo deja en el tema de iconos en Linux y lo
 * guarda para el próximo arranque (`src-tauri/src/icono.rs`).
 */

/** Las mismas mezclas que el degradado y la carta del `<Logo>`. */
const MEZCLAS: Record<keyof ColoresIconoHex, string> = {
	claro: "color-mix(in oklab, var(--primary) 80%, black)",
	medio: "color-mix(in oklab, var(--primary) 55%, black)",
	oscuro: "color-mix(in oklab, var(--primary) 35%, black)",
	figura: "color-mix(in oklab, var(--primary) 8%, white)",
};

/**
 * Un color CSS cualquiera (`oklch(…)`, `rgb(…)`…) a `#rrggbb`, a través de un
 * lienzo: así vale lo que devuelva `getComputedStyle` en cada motor.
 */
function aHex(css: string, ctx: CanvasRenderingContext2D): string | null {
	ctx.clearRect(0, 0, 1, 1);
	// Un color que el lienzo no entienda deja el de antes: entonces, nada (y no un icono negro).
	ctx.fillStyle = "#010203";
	ctx.fillStyle = css;
	if (ctx.fillStyle === "#010203") return null;
	ctx.fillRect(0, 0, 1, 1);
	const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
	if (!a) return null;
	return `#${[r, g, b].map((n) => n.toString(16).padStart(2, "0")).join("")}`;
}

/** Los colores del logo con el tema de ahora, o `null` si no se pueden leer. */
export function coloresDelTema(): ColoresIconoHex | null {
	const lienzo = document.createElement("canvas");
	lienzo.width = 1;
	lienzo.height = 1;
	const ctx = lienzo.getContext("2d", { willReadFrequently: true });
	if (!ctx) return null;
	const muestra = document.createElement("span");
	muestra.style.display = "none";
	document.body.append(muestra);
	try {
		const salida: Partial<ColoresIconoHex> = {};
		for (const [k, mezcla] of Object.entries(MEZCLAS) as [keyof ColoresIconoHex, string][]) {
			muestra.style.color = "";
			muestra.style.color = mezcla;
			const hex = aHex(getComputedStyle(muestra).color, ctx);
			if (!hex) return null;
			salida[k] = hex;
		}
		return salida as ColoresIconoHex;
	} finally {
		muestra.remove();
	}
}

let iniciado = false;

/** Empieza a seguir el tema (una vez; lo llama `main.tsx` en el escritorio). */
export function iniciarIconoVivo(): void {
	if (iniciado) return;
	iniciado = true;
	let ultimo = "";
	let espera: number | undefined;
	const enviar = () => {
		const c = coloresDelTema();
		if (!c) return;
		const clave = JSON.stringify(c);
		if (clave === ultimo) return;
		ultimo = clave;
		void call("app_icono", { colores: c }).catch(() => {
			// Sin icono nuevo no pasa nada; se reintenta en el próximo cambio.
			ultimo = "";
		});
	};
	// Un cambio de tema toca varios atributos seguidos: se espera a que acabe.
	const pronto = () => {
		window.clearTimeout(espera);
		espera = window.setTimeout(enviar, 250);
	};
	new MutationObserver(pronto).observe(document.documentElement, {
		attributes: true,
		attributeFilter: ["class", "data-theme", "data-skin", "style"],
	});
	window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", pronto);
	pronto();
}
