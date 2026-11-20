import { useEffect, useRef, useState } from "react";
import { useUi } from "@/stores/ui-store";

/** Lo que duran las entradas y salidas de las barras de la mesa, en ms (`duration-200`). */
export const DURACION_SALIDA = 200;

/**
 * **Montar con salida animada.** Algo que se esconde sigue montado lo que dura
 * su animación de salida (`saliendo`) y después se quita. `animar` solo es
 * cierto tras un cambio: lo que ya estaba al abrir la mesa no entra animado.
 * Con «Reducir animaciones» se quita al momento y sin animación.
 */
export function useConSalida(visible: boolean): {
	montado: boolean;
	saliendo: boolean;
	animar: boolean;
} {
	const reducir = useUi((s) => s.mesa.reducirAnimaciones);
	const [montado, setMontado] = useState(visible);
	const [animar, setAnimar] = useState(false);
	const antes = useRef(visible);
	useEffect(() => {
		if (antes.current === visible) return;
		antes.current = visible;
		setAnimar(!reducir);
		if (visible || reducir) {
			setMontado(visible);
			return;
		}
		const t = window.setTimeout(() => setMontado(false), DURACION_SALIDA);
		return () => window.clearTimeout(t);
	}, [visible, reducir]);
	return { montado: montado || visible, saliendo: !visible && montado, animar: animar && !reducir };
}
