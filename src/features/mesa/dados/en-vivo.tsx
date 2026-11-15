import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useUi } from "@/stores/ui-store";
import { useSinAnimaciones } from "../fps";
import { claveEstilo, type Estilo } from "./aspecto";
import type { Tirada } from "./cola";
import { DadosCss } from "./css3d";
import type { EscenaDados } from "./escena3d";
import { nivelReal } from "./nivel";
import { DadosPlanos, PiezaQuieta } from "./plano";
import { golpeDado, golpeMoneda, lanzarMoneda } from "./sonidos";
import { lienzoFondo, urlFondo } from "./texturas";

/**
 * **Una tirada como en la mesa**, en una caja cualquiera: la muestra de
 * Ajustes > Mesa y el último resultado del diálogo «Dados y monedas». Usa
 * lo mismo que la capa de la partida (`capa.tsx`): con el nivel de Ajustes,
 * en 3D con three.js, la física de Rapier y sus sombras; si no hay WebGL o
 * falla, en CSS 3D; o en 2D si se pidió. La física es determinista por la
 * semilla, así que la misma tirada cae siempre igual y en la cara del motor.
 * Las piezas se quedan en la mesa al parar.
 */
export function TiradaEnVivo({
	tirada,
	estilo,
	sonido = true,
	className,
}: {
	tirada: Tirada;
	estilo: Estilo;
	sonido?: boolean;
	className?: string;
}) {
	const pedido = useUi((s) => s.mesa.dados);
	const [fallo3d, setFallo3d] = useState(false);
	const sinAnimaciones = useSinAnimaciones();
	const nivel = nivelReal(pedido, fallo3d);
	const lienzo = useRef<HTMLCanvasElement>(null);
	const escena = useRef<EscenaDados | null>(null);
	const sonar = useRef(sonido);
	sonar.current = sonido;

	// El fondo de las caras, una vez por estilo.
	const clave = claveEstilo(estilo);
	const [fondo, setFondo] = useState<{
		clave: string;
		lienzo: HTMLCanvasElement;
		url: string;
	} | null>(null);
	useEffect(() => {
		let vivo = true;
		void lienzoFondo(estilo).then((l) => {
			if (vivo) setFondo((f) => (f?.clave === clave ? f : { clave, lienzo: l, url: urlFondo(l) }));
		});
		return () => {
			vivo = false;
		};
	}, [estilo, clave]);
	const fondoListo = fondo?.clave === clave ? fondo : null;

	// El 3D: la escena se crea la primera vez (three y Rapier van aparte).
	useEffect(() => {
		if (sinAnimaciones || nivel !== "3d" || !lienzo.current || !fondoListo) return;
		let vivo = true;
		const canvas = lienzo.current;
		void (async () => {
			try {
				if (!escena.current) {
					const { EscenaDados } = await import("./escena3d");
					if (!vivo) return;
					escena.current = new EscenaDados(canvas);
				}
				if (sonar.current && tirada.piezas.some((p) => p.forma === "moneda")) lanzarMoneda();
				await escena.current.tirar(tirada, estilo, fondoListo.lienzo, (moneda, fuerza) => {
					if (!sonar.current) return;
					if (moneda) golpeMoneda(fuerza);
					else golpeDado(fuerza);
				});
			} catch {
				// Sin WebGL o sin WebAssembly: CSS 3D.
				escena.current = null;
				if (vivo) setFallo3d(true);
			}
		})();
		return () => {
			vivo = false;
		};
	}, [sinAnimaciones, nivel, tirada, estilo, fondoListo]);

	// Al cambiar de tamaño, la cámara; al irse, la escena.
	useEffect(() => {
		const el = lienzo.current;
		if (!el) return;
		const obs = new ResizeObserver(() => escena.current?.medir());
		obs.observe(el);
		return () => {
			obs.disconnect();
			escena.current?.destruir();
			escena.current = null;
		};
	}, []);

	return (
		<div className={cn("relative overflow-hidden", className)}>
			<canvas
				ref={lienzo}
				className={cn("absolute inset-0 size-full", (sinAnimaciones || nivel !== "3d") && "hidden")}
			/>
			{fondoListo && sinAnimaciones && (
				// Con las animaciones reducidas, las piezas ya paradas.
				<div className="absolute inset-0 flex flex-wrap items-center justify-center gap-2">
					{tirada.piezas.map((p, i) => (
						<PiezaQuieta key={i} p={p} estilo={estilo} fondo={fondoListo.url} />
					))}
				</div>
			)}
			{fondoListo && !sinAnimaciones && nivel === "css" && (
				<DadosCss
					key={tirada.id}
					tirada={tirada}
					estilo={estilo}
					fondo={fondoListo.url}
					alAcabar={() => {}}
					sonido={sonido}
				/>
			)}
			{fondoListo && !sinAnimaciones && nivel === "2d" && (
				<DadosPlanos
					key={tirada.id}
					tirada={tirada}
					estilo={estilo}
					fondo={fondoListo.url}
					alAcabar={() => {}}
				/>
			)}
		</div>
	);
}
