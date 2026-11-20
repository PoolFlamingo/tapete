import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { cartasDe, cuantasEn, useMesa } from "@/stores/mesa-store";
import { hacer, zonaComun } from "./acciones";
import { CabeceraPanel } from "./cabecera-panel";
import { useMesaMovil } from "./formato-mesa";
import { caraVisible, ImagenCarta } from "./imagen-carta";
import { useLeerEnGrande } from "./leer-en-grande";
import { planeswalkPendiente } from "./planar";

/**
 * **El plano activo de Planechase**: la carta del plano, «Planeswalk» y el
 * dado planar con lo que cuesta la siguiente tirada. Sale solo con la
 * variante encendida (o si ya hay un plano activo).
 */
export function PanelPlanar() {
	const hay = useMesa(
		(s) => (s.variantes?.planechase ?? false) || cuantasEn(s, zonaComun("planoActivo")) > 0,
	);
	return hay ? <PlanoActivo /> : null;
}

function PlanoActivo() {
	const { t } = useLang();
	const activo = useMesa(useShallow((s) => cartasDe(s, zonaComun("planoActivo"))));
	const carta = useMesa((s) => (activo[0] === undefined ? undefined : s.cartas[activo[0]]));
	const enMazo = useMesa((s) => cuantasEn(s, zonaComun("planos")));
	const tiradas = useMesa((s) => s.general?.turno.tiradasPlanar ?? 0);
	const pendiente = useMesa((s) => planeswalkPendiente(s.registro));
	const quieta = useMesa((s) => s.general?.mulligan != null);
	const leer = useLeerEnGrande(carta?.id ?? null);
	const movil = useMesaMovil();
	const [plegado, setPlegado] = useState(false);
	return (
		<div
			className="flex w-48 flex-col gap-1.5 rounded-lg border border-white/10 bg-black/60 p-2 text-white shadow-lg backdrop-blur-md"
			data-tutorial="mesa-plano"
		>
			<CabeceraPanel
				titulo={t("Plano activo")}
				subtitulo={t("{cuantos} en el mazo", { cuantos: enMazo })}
				plegable={movil}
				plegado={movil && plegado}
				onPlegar={setPlegado}
			/>
			{!(movil && plegado) && (
				<>
					<div
						className="aspect-[88/63] w-full overflow-hidden rounded-md border border-white/15 bg-black/40"
						{...leer}
					>
						{carta ? (
							<ImagenCarta imagen={caraVisible(carta)} ancho={244} nombre={carta.origen?.nombre} />
						) : (
							<p className="flex size-full items-center justify-center p-2 text-center text-[11px] text-white/60">
								{t("No hay planos: añade planos al deck.")}
							</p>
						)}
					</div>
					<div className="grid grid-cols-2 gap-1.5">
						<Button
							size="sm"
							variant={pendiente ? "default" : "secondary"}
							className={cn("px-1.5 text-[11px]", pendiente && "animate-pulse")}
							disabled={quieta || (activo.length === 0 && enMazo === 0)}
							onClick={() => void hacer({ que: "planeswalk" })}
						>
							{t("Planeswalk")}
						</Button>
						<Button
							size="sm"
							variant="secondary"
							className="px-1.5 text-[11px]"
							disabled={quieta}
							onClick={() => void hacer({ que: "dadoPlanar" })}
						>
							{t("Dado planar")}
						</Button>
					</div>
					<p className="text-center text-[10px] text-white/60">
						{tiradas === 0
							? t("La próxima tirada es gratis.")
							: t("La próxima tirada cuesta {coste}.", { coste: tiradas })}
					</p>
				</>
			)}
		</div>
	);
}
