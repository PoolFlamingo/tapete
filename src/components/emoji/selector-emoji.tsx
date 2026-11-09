import { Picker } from "emoji-mart";
import { useEffect, useRef } from "react";
import { useLang } from "@/components/language-provider";
import { cn } from "@/lib/utils";
import { idiomaDelSelector, traduccionDelSelector } from "./idioma";

/**
 * **El selector de EmojiMart**, en su propio trozo: solo se carga la primera
 * vez que se pulsa el botón (`boton-emoji.tsx` lo pide con `lazy`).
 *
 * 🛑 Nada de internet (la CSP solo deja `ipc:`, y la vista web no sale fuera):
 * - los datos, del paquete (`@emoji-mart/data`, juego 15 nativo);
 * - la traducción, del paquete y **siempre** (sin ella la pide a la CDN);
 * - el juego `native`: los emoji los pinta la letra del sistema, la misma que
 *   pinta el chat, así que se ve en el selector lo mismo que saldrá en el
 *   mensaje. Los otros juegos (google…) bajan sus imágenes de jsdelivr.
 *
 * Se usa el `Picker` de `emoji-mart` directamente (un elemento con su propia
 * sombra) y no `@emoji-mart/react`, que pide React ≤ 18 y solo hace esto mismo.
 */

let datos: Promise<unknown> | null = null;
function datosNativos(): Promise<unknown> {
	datos ??= import("@emoji-mart/data/sets/15/native.json").then((m) => m.default);
	return datos;
}

/** Un color CSS cualquiera (oklch incluido) como «r, g, b», lo que pide emoji-mart. */
function rgb(color: string): [number, number, number] | null {
	const lienzo = document.createElement("canvas");
	lienzo.width = 1;
	lienzo.height = 1;
	const ctx = lienzo.getContext("2d", { willReadFrequently: true });
	if (!ctx) return null;
	ctx.fillStyle = "#000";
	ctx.fillStyle = color;
	ctx.fillRect(0, 0, 1, 1);
	const [r = 0, g = 0, b = 0] = ctx.getImageData(0, 0, 1, 1).data;
	return [r, g, b];
}

/** Los colores del tema que se ve ahora, leídos de los tokens de la app. */
function coloresDelTema(): { variables: Record<string, string>; oscuro: boolean } {
	const sonda = document.createElement("div");
	sonda.style.display = "none";
	document.body.appendChild(sonda);
	const leer = (token: string) => {
		sonda.style.color = `var(${token})`;
		return rgb(getComputedStyle(sonda).color);
	};
	const fondo = leer("--popover");
	const texto = leer("--popover-foreground");
	const acento = leer("--primary");
	const campo = leer("--muted");
	const raiz = getComputedStyle(document.documentElement);
	sonda.remove();
	const v: Record<string, string> = {
		"--font-family": raiz.getPropertyValue("--font-sans").trim() || "sans-serif",
		"--border-radius": raiz.getPropertyValue("--radius").trim() || "14px",
		"--shadow": "none",
	};
	const triple = (c: [number, number, number] | null) => (c ? c.join(", ") : null);
	for (const [nombre, c] of [
		["--rgb-background", fondo],
		["--rgb-color", texto],
		["--rgb-accent", acento],
		["--rgb-input", campo],
	] as const) {
		const t = triple(c);
		if (t) v[nombre] = t;
	}
	if (texto) v["--color-border"] = `rgba(${texto.join(", ")}, .1)`;
	if (texto) v["--color-border-over"] = `rgba(${texto.join(", ")}, .2)`;
	// Claro u oscuro por el brillo del fondo, no por el modo: los temas mandan.
	const oscuro = fondo ? 0.2126 * fondo[0] + 0.7152 * fondo[1] + 0.0722 * fondo[2] < 128 : true;
	return { variables: v, oscuro };
}

export default function SelectorEmoji({
	alElegir,
	ancho,
}: {
	alElegir: (emoji: string) => void;
	/** `"lleno"`: el ancho del contenedor (la hoja del móvil); si no, el suyo. */
	ancho?: "lleno";
}) {
	const { lang } = useLang();
	const caja = useRef<HTMLDivElement>(null);
	// El último callback, sin volver a montar el selector cuando cambia.
	const elegir = useRef(alElegir);
	elegir.current = alElegir;

	useEffect(() => {
		let vivo = true;
		let el: HTMLElement | null = null;
		const idioma = idiomaDelSelector(lang);
		void Promise.all([datosNativos(), traduccionDelSelector(idioma)]).then(([data, i18n]) => {
			if (!vivo || !caja.current) return;
			const { variables, oscuro } = coloresDelTema();
			el = new Picker({
				data,
				i18n,
				locale: idioma,
				set: "native",
				theme: oscuro ? "dark" : "light",
				previewPosition: "none",
				skinTonePosition: "search",
				navPosition: "top",
				autoFocus: false,
				dynamicWidth: ancho === "lleno",
				onEmojiSelect: (e: { native?: string }) => {
					if (e.native) elegir.current(e.native);
				},
			}) as unknown as HTMLElement;
			for (const [k, v] of Object.entries(variables)) el.style.setProperty(k, v);
			if (ancho === "lleno") {
				el.style.width = "100%";
				// En la hoja del móvil, lo que deja la hoja (`Hoja`: como mucho 85 dvh)
				// menos su cabecera (3 rem) y el relleno: de lado solo hay ~411 dp y con
				// 435 px se anidaban dos scrolls.
				el.style.height = "min(435px, calc(85dvh - 4.25rem))";
			}
			caja.current.appendChild(el);
		});
		return () => {
			vivo = false;
			el?.remove();
		};
	}, [lang, ancho]);

	return (
		<div
			ref={caja}
			className={cn("flex w-full justify-center", ancho !== "lleno" && "min-h-[435px]")}
		/>
	);
}
