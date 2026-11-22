/**
 * El botón «Identificar por foto» (Buscar y «Añadir carta»): en el móvil abre
 * el escáner en vivo; en el escritorio, el diálogo para soltar o elegir fotos.
 */
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { useEsMovil } from "@/lib/plataforma";
import { type ModoReconocer, useReconocer } from "@/stores/reconocer-store";

export function BotonReconocer({
	modo,
	antes,
	conTexto = false,
}: {
	modo: ModoReconocer;
	/** Lo que hay que hacer antes de abrir (cerrar «Añadir carta»). */
	antes?: () => void;
	/** Con el texto al lado (si no, solo el icono con su ayuda). */
	conTexto?: boolean;
}) {
	const { t } = useLang();
	const movil = useEsMovil();
	const abrir = () => {
		antes?.();
		useReconocer.getState().abrir(modo, movil);
	};
	return (
		<WithTooltip content={t("Identificar por foto")}>
			<Button
				size="sm"
				variant="outline"
				aria-label={t("Identificar por foto")}
				data-tutorial="identificar-foto"
				onClick={abrir}
			>
				<Icono icon="lucide:scan-line" />
				{conTexto && <span className="max-sm:sr-only">{t("Por foto")}</span>}
			</Button>
		</WithTooltip>
	);
}
