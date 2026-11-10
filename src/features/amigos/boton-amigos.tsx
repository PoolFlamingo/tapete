import { FilaMenu } from "@/components/hoja-menu";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { pendientes, useAmigos } from "@/stores/amigos-store";

function Cuenta({ n }: { n: number }) {
	if (n === 0) return null;
	return (
		<span className="absolute -top-1 -right-1 min-w-4 rounded-full bg-primary px-1 text-center text-[10px] leading-4 font-semibold text-primary-foreground tabular-nums">
			{n > 99 ? "99+" : n}
		</span>
	);
}

/** La entrada a «Amigos» en la barra de arriba (escritorio), con lo que hay por mirar. */
export function BotonAmigos() {
	const { t } = useLang();
	const n = useAmigos((s) => pendientes(s.estado));
	const etiqueta = n > 0 ? `${t("Amigos")}: ${n}` : t("Amigos");
	return (
		<WithTooltip content={etiqueta}>
			<Button
				size="icon-sm"
				variant="ghost"
				className="relative"
				aria-label={etiqueta}
				data-tutorial="amigos"
				onClick={() => useAmigos.getState().abrir()}
			>
				<Icono icon="lucide:users" className="size-4" />
				<Cuenta n={n} />
			</Button>
		</WithTooltip>
	);
}

/** La entrada a «Amigos» en la hoja «Más» del móvil. */
export function FilaAmigos() {
	const { t } = useLang();
	const n = useAmigos((s) => pendientes(s.estado));
	return (
		<FilaMenu
			icono="lucide:users"
			texto={t("Amigos")}
			onClick={() => useAmigos.getState().abrir()}
			extra={
				n > 0 ? (
					<span className="rounded-full bg-primary px-1.5 text-[10px] font-semibold text-primary-foreground tabular-nums">
						{n}
					</span>
				) : undefined
			}
		/>
	);
}

/** El punto del «Más» del móvil cuando hay algo de amigos por mirar. */
export function PuntoAmigos() {
	const n = useAmigos((s) => pendientes(s.estado));
	if (n === 0) return null;
	return <span className="absolute -top-0.5 -left-1 size-2 rounded-full bg-primary" />;
}
