import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";

/**
 * La cabecera de un panel de la mesa (plano activo, planes): título y subtítulo.
 * En el móvil, donde el panel tapa medio campo, es un botón que lo pliega a
 * solo esta línea (`plegado`); en escritorio, texto sin más.
 */
export function CabeceraPanel({
	titulo,
	subtitulo,
	plegable,
	plegado,
	onPlegar,
}: {
	titulo: string;
	subtitulo: string;
	plegable: boolean;
	plegado: boolean;
	onPlegar: (si: boolean) => void;
}) {
	const { t } = useLang();
	const texto = (
		<span className="flex min-w-0 flex-1 flex-col text-left text-[10px] leading-tight">
			<span className="font-semibold tracking-wider uppercase">{titulo}</span>
			<span className="text-white/60">{subtitulo}</span>
		</span>
	);
	if (!plegable) return <div className="flex">{texto}</div>;
	return (
		<button
			type="button"
			aria-expanded={!plegado}
			aria-label={
				plegado ? t("Desplegar {panel}", { panel: titulo }) : t("Plegar {panel}", { panel: titulo })
			}
			onClick={() => onPlegar(!plegado)}
			className="flex min-h-8 items-center gap-1.5"
		>
			{texto}
			<Icono
				icon={plegado ? "lucide:chevron-down" : "lucide:chevron-up"}
				className="size-4 shrink-0"
			/>
		</button>
	);
}
