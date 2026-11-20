import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useMesa } from "@/stores/mesa-store";
import { useRed } from "@/stores/red-store";
import { hacer } from "../acciones";
import { ESPERA_SALTAR_MS, segundosAusente, siguienteVivo } from "../cuentas-muerte";
import { useMesaMovil } from "../formato-mesa";

/**
 * **Quien se ha ido de la partida** (G8, en red): un aviso con quién falta.
 * Si es a quien le toca jugar, a los 30 s el siguiente jugador ve «Saltar el
 * turno de X» y los demás, quién puede hacerlo. Con la partida en el
 * mulligan, el anfitrión se queda por él con su mano pasado un minuto (lo
 * hace Rust; aquí solo se cuenta).
 */
export function Ausentes() {
	const { t } = useLang();
	const movil = useMesaMovil();
	const ausentes = useRed((s) => s.ausentes);
	const papel = useRed((s) => s.papel);
	const asientos = useMesa(useShallow((s) => s.asientos));
	const jugadores = useMesa((s) => s.jugadores);
	const activo = useMesa((s) => s.general?.turno.activo ?? 0);
	const yo = useMesa((s) => s.yo ?? 0);
	const mulligan = useMesa((s) => s.general?.mulligan != null);
	const fin = useMesa((s) => s.general?.fin != null);
	const [ahora, setAhora] = useState(() => Date.now());
	// Cada aviso se pliega a una pastilla (tapa el campo y quien se fue puede tardar en volver).
	const [plegados, setPlegados] = useState<number[]>([]);
	useEffect(() => {
		if (ausentes.length === 0) return;
		const id = window.setInterval(() => setAhora(Date.now()), 1000);
		return () => window.clearInterval(id);
	}, [ausentes.length]);
	const filas = ausentes.filter(
		(a) => jugadores[a.jugador] && jugadores[a.jugador]?.eliminado == null,
	);
	if (papel === null || fin || filas.length === 0) return null;
	// Lo salta el siguiente que sigue en la partida y conectado (si el de después
	// también se fue, nadie podría).
	const vivos = asientos.filter(
		(id) => jugadores[id]?.eliminado == null && !ausentes.some((a) => a.jugador === id),
	);
	const siguiente = siguienteVivo(asientos, vivos, activo);
	return (
		<div
			className={cn(
				"absolute right-3 z-[7500] flex max-w-72 flex-col gap-1.5",
				// En el móvil, «Pasar turno» ocupa la esquina de arriba a la derecha.
				movil ? "top-[4.5rem]" : "top-14",
			)}
		>
			{filas.map((a) => {
				const quien = jugadores[a.jugador]?.nombre ?? "";
				const seg = segundosAusente(a.haceMs, a.visto, ahora);
				const esElTurno = a.jugador === activo && !mulligan;
				const puedeSaltar = esElTurno && seg * 1000 >= ESPERA_SALTAR_MS;
				// Si me toca saltar su turno, el aviso se abre aunque estuviera plegado.
				const meToca = puedeSaltar && siguiente === yo && papel !== "espectador";
				if (plegados.includes(a.jugador) && !meToca) {
					return (
						<button
							key={a.jugador}
							type="button"
							aria-label={t("{quien} está desconectado.", { quien })}
							onClick={() => setPlegados(plegados.filter((x) => x !== a.jugador))}
							className="flex h-8 items-center gap-1.5 self-end rounded-full border border-amber-500/40 bg-black/85 px-2.5 text-[11px] text-white shadow-lg backdrop-blur-md"
						>
							<Icono icon="lucide:user-x" className="size-3.5 shrink-0 text-amber-400" />
							<span className="max-w-24 truncate">{quien}</span>
						</button>
					);
				}
				return (
					<div
						key={a.jugador}
						className="flex flex-col gap-1.5 rounded-xl border border-amber-500/40 bg-black/85 p-2.5 text-xs text-white shadow-lg backdrop-blur-md"
					>
						<div className="flex items-center gap-2">
							<Icono icon="lucide:user-x" className="size-3.5 shrink-0 text-amber-400" />
							<span className="min-w-0 flex-1">{t("{quien} está desconectado.", { quien })}</span>
							<button
								type="button"
								aria-label={t("Plegar el aviso")}
								onClick={() => setPlegados([...plegados, a.jugador])}
								className="-m-1 flex size-7 shrink-0 items-center justify-center rounded-md text-white/70 hover:bg-white/10 hover:text-white"
							>
								<Icono icon="lucide:minus" className="size-4" />
							</button>
						</div>
						{mulligan && (
							<p className="text-white/70">{t("Se quedará con su mano si no vuelve.")}</p>
						)}
						{esElTurno && !puedeSaltar && (
							<p className="flex items-center gap-1.5 text-white/70">
								<Icono icon="lucide:loader-2" className="size-3 animate-spin" />
								{t("Su turno se podrá saltar en {segundos} s.", {
									segundos: Math.max(0, Math.ceil(ESPERA_SALTAR_MS / 1000) - seg),
								})}
							</p>
						)}
						{puedeSaltar &&
							(meToca ? (
								<Button
									size="sm"
									onClick={() => void hacer({ que: "saltarTurno", jugador: a.jugador })}
								>
									{t("Saltar el turno de {quien}", { quien })}
								</Button>
							) : (
								siguiente !== null && (
									<p className="text-white/70">
										{t("{quien} puede saltar su turno.", {
											quien: jugadores[siguiente]?.nombre ?? "",
										})}
									</p>
								)
							))}
					</div>
				);
			})}
		</div>
	);
}
