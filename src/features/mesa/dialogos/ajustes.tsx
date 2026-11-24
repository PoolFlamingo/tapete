import { DialogoAdaptable } from "@/components/dialogo-adaptable";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogCuerpo,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { AjustesMesa } from "@/features/settings/mesa";
import { useRed } from "@/stores/red-store";
import { usePantalla } from "../pantalla";

const cerrar = () => usePantalla.getState().abrir(null);

/**
 * **Ajustes de la mesa a media partida**: los mismos de Ajustes > Mesa
 * (`settings/mesa.tsx`, que se aplican al momento) y el tapete, sin salir de
 * la partida. El tapete se lleva a la partida (`mesa_tapete_partida`) y los
 * demás jugadores lo reciben como un evento más; el resto es de cada pantalla.
 */
export function AjustesMesaDialog() {
	const { t } = useLang();
	const juega = useRed((s) => s.papel) !== "espectador";
	return (
		<Dialog open onOpenChange={(o) => !o && cerrar()}>
			<DialogoAdaptable className="sm:max-w-xl" cambiosSinGuardar={false}>
				<DialogHeader>
					<DialogTitle>{t("Ajustes de la mesa")}</DialogTitle>
					<DialogDescription>
						{t("Cambian al momento. Tu tapete lo verán también los demás jugadores.")}
					</DialogDescription>
				</DialogHeader>
				<DialogCuerpo>
					<div className="flex flex-col gap-2.5 p-1">
						{juega && (
							<Button
								size="sm"
								variant="outline"
								className="self-start"
								onClick={() => usePantalla.getState().abrir({ tipo: "tapete" })}
							>
								<Icono icon="lucide:image" className="size-3.5" />
								{t("Cambiar el tapete…")}
							</Button>
						)}
						<AjustesMesa fija enPartida />
					</div>
				</DialogCuerpo>
			</DialogoAdaptable>
		</Dialog>
	);
}
