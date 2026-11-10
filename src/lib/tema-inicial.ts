import { THEMES, type ThemeName } from "@/lib/themes";

/**
 * **El tema puesto antes de montar la pantalla.** El `ThemeProvider` pone el
 * tema, el modo oscuro y la superficie en `<html>` desde un efecto, que va
 * después del primer pintado: hasta entonces la página sale con los colores
 * por defecto (claros) y, con la ventana ya enseñada, se ve un destello
 * blanco. Esto hace lo mismo de una vez, al arrancar.
 *
 * 🛑 Las claves y los valores de fábrica son los de
 * `components/theme-provider.tsx` (de solo lectura, heredado de judlex): si
 * cambian allí, aquí también. Salvo el tema: el de fábrica de Tapete es verde,
 * como el paño del logo (Pablo, 10/11/2026), y el provider (que trae
 * `amber-minimal`) no se puede tocar; así que, sin tema elegido, se deja
 * guardado este antes de montar y el provider lo lee como uno elegido.
 */
export const TEMA_FABRICA: ThemeName = "nature";

function guardar(clave: string, valor: string) {
	try {
		localStorage.setItem(clave, valor);
	} catch {
		// Sin almacenamiento, el provider pondrá el suyo.
	}
}

function leer(clave: string): string | null {
	try {
		return localStorage.getItem(clave);
	} catch {
		return null;
	}
}

export function ponerTemaAntesDeMontar() {
	const raiz = document.documentElement;
	const guardado = leer("proxybuilder.theme");
	// Como el provider: solo un tema que esté en `THEMES` (los instalados ya
	// entraron con `prepararTemasInstalados`).
	const valido = guardado !== null && THEMES.some((t) => t.name === guardado);
	const tema = valido
		? guardado
		: THEMES.some((t) => t.name === TEMA_FABRICA)
			? TEMA_FABRICA
			: (THEMES[0]?.name ?? TEMA_FABRICA);
	if (!valido) guardar("proxybuilder.theme", tema);
	const modo = leer("proxybuilder.mode") || "dark";
	const oscuro =
		modo === "system"
			? window.matchMedia?.("(prefers-color-scheme: dark)").matches === true
			: modo === "dark";
	raiz.setAttribute("data-theme", tema);
	raiz.classList.toggle("dark", oscuro);
	// La superficie: el tema «base» va limpio, sin skin, fondo ni material.
	if (tema === "base") return;
	const skin = leer("proxybuilder.skin") || "none";
	const fondo = leer("proxybuilder.bg") || "none";
	const material = leer("proxybuilder.material") || "matte";
	if (skin !== "none") raiz.setAttribute("data-skin", skin);
	if (fondo !== "none") raiz.setAttribute("data-bg", fondo);
	if (material !== "solid") raiz.setAttribute("data-material", material);
}
