import type { NivelDados } from "@/stores/ui-store";

/** Si hay WebGL2 (sin él, los dados van en CSS 3D). Se mira una vez. */
let webgl2: boolean | null = null;
function hayWebGL2(): boolean {
	if (webgl2 === null) {
		try {
			webgl2 = !!document.createElement("canvas").getContext("webgl2");
		} catch {
			webgl2 = false;
		}
	}
	return webgl2;
}

/**
 * El nivel de los dados que se usa: el de Ajustes, o el mejor que aguante el
 * equipo. El mismo en la mesa (`capa.tsx`), en la muestra de Ajustes y en el
 * diálogo de los dados (`en-vivo.tsx`).
 */
export function nivelReal(pedido: NivelDados, fallo3d: boolean): Exclude<NivelDados, "auto"> {
	if (pedido === "auto") return hayWebGL2() && !fallo3d ? "3d" : "css";
	if (pedido === "3d" && fallo3d) return "css";
	return pedido;
}
