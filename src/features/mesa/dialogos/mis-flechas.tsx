import { useEffect, useMemo } from "react";
import type { Extremo } from "@/bindings/Extremo";
import type { Flecha } from "@/bindings/Flecha";
import { DialogoAdaptable } from "@/components/dialogo-adaptable";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogCuerpo,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { cn } from "@/lib/utils";
import { NINGUNA, useMesa } from "@/stores/mesa-store";
import { hacer } from "../acciones";
import { nombreFantasma } from "../movil/cuentas-vida";
import { usePantalla } from "../pantalla";

const cerrar = () => usePantalla.getState().abrir(null);

/**
 * **Mis flechas** (clic derecho en el campo vacío): cada flecha de ataque u
 * objetivo que he puesto, para quitar solo la que sobra (`quitarFlecha`) en
 * vez de todas. La que se señala se resalta en la mesa (`flechaResaltada`,
 * que lee `flechas.tsx`).
 */
export function MisFlechasDialog() {
	const { t } = useLang();
	const yo = useMesa((s) => s.yo);
	const todas = useMesa((s) => s.general?.flechas ?? NINGUNA);
	const cartas = useMesa((s) => s.cartas);
	const jugadores = useMesa((s) => s.jugadores);
	const otros = useMesa((s) => s.general?.fantasmasExtra ?? NINGUNA).length;
	const resaltada = usePantalla((s) => s.flechaResaltada);
	const resaltar = usePantalla((s) => s.setFlechaResaltada);
	// El sitio de cada una en `general.flechas`, que es lo que pinta la mesa.
	const mias = useMemo(
		() => todas.flatMap((f, i) => (yo !== null && f.deJugador === yo ? [{ f, i }] : [])),
		[todas, yo],
	);
	// Al cerrar, ninguna resaltada.
	useEffect(() => () => resaltar(null), [resaltar]);

	const nombre = (e: Extremo): string => {
		switch (e.tipo) {
			case "carta":
				return cartas[e.id]?.origen?.nombre ?? t("una carta");
			case "jugador":
				return e.id === yo ? t("Tú") : (jugadores[e.id]?.nombre ?? "?");
			case "fantasma":
				return nombreFantasma(t, null, otros);
			case "otroFantasma":
				return nombreFantasma(t, e.id, otros);
		}
	};
	const quitar = (f: Flecha) => {
		// Los sitios cambian al quitar una: la resaltada ya no sería esa.
		resaltar(null);
		void hacer({ que: "quitarFlecha", de: f.de, a: f.a, tipo: f.tipo });
	};

	return (
		<Dialog open onOpenChange={(o) => !o && cerrar()}>
			<DialogoAdaptable className="sm:max-w-md" cambiosSinGuardar={false}>
				<DialogHeader>
					<DialogTitle>{t("Mis flechas")}</DialogTitle>
					<DialogDescription>
						{t("Quita solo la que sobra. La que señales se resalta en la mesa.")}
					</DialogDescription>
				</DialogHeader>
				<DialogCuerpo>
					{mias.length === 0 ? (
						<p className="p-2.5 text-center text-xs text-muted-foreground">
							{t("No tienes flechas en la mesa.")}
						</p>
					) : (
						<ul className="flex flex-col gap-1 p-1">
							{mias.map(({ f, i }) => {
								const ataque = f.tipo === "ataque";
								return (
									<li
										key={i}
										className={cn(
											"flex min-w-0 items-center gap-1 rounded-md border pr-1 text-xs transition-colors",
											resaltada === i && "border-primary bg-primary/10",
										)}
									>
										<button
											type="button"
											// táctil: tocar la fila la resalta (onClick); la papelera de al lado la quita.
											onPointerEnter={(e) => e.pointerType === "mouse" && resaltar(i)}
											onPointerLeave={(e) => e.pointerType === "mouse" && resaltar(null)}
											onFocus={() => resaltar(i)}
											onClick={() => resaltar(i)}
											className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
										>
											<Icono
												icon={ataque ? "game-icons:crossed-swords" : "game-icons:on-target"}
												className="size-4 shrink-0"
											/>
											<span className="flex min-w-0 flex-1 flex-col">
												<span className="truncate font-medium">
													{nombre(f.de)} → {nombre(f.a)}
												</span>
												<span className="text-[10px] text-muted-foreground">
													{ataque ? t("Ataque") : t("Objetivo")}
												</span>
											</span>
										</button>
										<WithTooltip content={t("Quitar esta flecha")}>
											<Button
												size="icon"
												variant="ghost"
												className="size-7 shrink-0"
												aria-label={t("Quitar esta flecha")}
												onClick={() => quitar(f)}
											>
												<Icono icon="lucide:trash-2" className="size-3.5" />
											</Button>
										</WithTooltip>
									</li>
								);
							})}
						</ul>
					)}
				</DialogCuerpo>
				<DialogFooter>
					<Button
						size="sm"
						variant="outline"
						disabled={mias.length === 0}
						onClick={() => {
							void hacer({ que: "quitarFlechas", de: null });
							cerrar();
						}}
					>
						<Icono icon="lucide:unlink" />
						{t("Quitar todas")}
					</Button>
				</DialogFooter>
			</DialogoAdaptable>
		</Dialog>
	);
}
