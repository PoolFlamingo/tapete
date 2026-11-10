import { useEffect } from "react";
import { toast } from "sonner";
import type { EnlaceEntrante } from "@/bindings/EnlaceEntrante";
import { paraTraducir, traduccionSuelta } from "@/components/language-provider";
import { call, on } from "@/lib/ipc";
import { useAmigos } from "@/stores/amigos-store";
import { useMesa } from "@/stores/mesa-store";

/**
 * Llega un enlace (`tapete://` o `proxybuilder://`; Rust ya comprobó el código). Uno de
 * partida lleva a «Jugar en red» con el código puesto (con una partida en marcha no se hace
 * nada y se avisa); uno de amigo abre «Amigos» con el código en «Añadir». Unirse o pedir
 * amistad lo pulsa la persona. Devuelve si hay que ir a «Jugar».
 */
export function aceptarEnlace(e: EnlaceEntrante): boolean {
	if (e.tipo === "amigo") {
		useAmigos.getState().pedirAnadir(e.codigo);
		useAmigos.getState().abrir();
		return false;
	}
	if (useMesa.getState().activa) {
		toast(
			traduccionSuelta(paraTraducir("Hay una partida en marcha: sal de ella para usar el enlace.")),
		);
		return false;
	}
	useAmigos.getState().pedirUnirse(e.codigo);
	return true;
}

/**
 * Escucha los enlaces (el que abrió la app, que Rust guardó, y los que lleguen después) y
 * lleva a la pestaña «Jugar» con `irAMesa` cuando toca.
 */
export function useEnlacesEntrantes(irAMesa: () => void) {
	useEffect(() => {
		let soltar: (() => void) | undefined;
		let vivo = true;
		const llega = (e: EnlaceEntrante) => {
			if (aceptarEnlace(e)) irAMesa();
		};
		void on("enlace://entrante", llega).then((f) => {
			if (vivo) soltar = f;
			else f();
		});
		call("enlace_pendiente")
			.then((e) => e && vivo && llega(e))
			.catch(() => {});
		return () => {
			vivo = false;
			soltar?.();
		};
	}, []);
}
