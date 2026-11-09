import { lazy, Suspense, useState } from "react";
import { HojaMenu } from "@/components/hoja-menu";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { useFormato } from "@/lib/formato";

/** El selector va en su propio trozo: se baja la primera vez que se abre. */
const SelectorEmoji = lazy(() => import("./selector-emoji"));

function Cargando() {
	return (
		<div className="flex h-[435px] w-[352px] max-w-full items-center justify-center text-muted-foreground">
			<Icono icon="lucide:loader-2" className="size-5 animate-spin" />
		</div>
	);
}

/**
 * **El botón de emojis de los chats** (EmojiMart): en el escritorio y la
 * tableta abre un `Popover` junto al botón; en el móvil, una `HojaMenu` que
 * sube con el selector a todo el ancho (un `Popover` de 352 px no cabe de lado
 * con el teclado, y los menús del móvil suben como hoja).
 *
 * El emoji elegido se lo da a `alElegir` (que lo mete donde está el cursor,
 * `insertarEnCursor`); el selector se queda abierto en el escritorio para
 * poner varios seguidos, y la hoja del móvil se cierra (tapa el campo).
 */
export function BotonEmoji({
	alElegir,
	disabled,
	size = "icon",
	variant = "outline",
}: {
	alElegir: (emoji: string) => void;
	disabled?: boolean;
	size?: "icon" | "icon-xs" | "icon-sm";
	variant?: "outline" | "ghost";
}) {
	const { t } = useLang();
	const { tamano } = useFormato();
	const [abierto, setAbierto] = useState(false);
	const etiqueta = t("Emojis");
	const boton = (
		<Button
			size={size}
			variant={variant}
			disabled={disabled}
			aria-label={etiqueta}
			onClick={tamano === "movil" ? () => setAbierto(true) : undefined}
		>
			<Icono icon="lucide:smile-plus" className="size-4" />
		</Button>
	);

	if (tamano === "movil") {
		return (
			<>
				{boton}
				<HojaMenu
					abierta={abierto}
					onCerrar={() => setAbierto(false)}
					titulo={etiqueta}
					cuerpoClassName="p-2"
				>
					{abierto && (
						<Suspense fallback={<Cargando />}>
							<SelectorEmoji
								ancho="lleno"
								alElegir={(e) => {
									alElegir(e);
									setAbierto(false);
								}}
							/>
						</Suspense>
					)}
				</HojaMenu>
			</>
		);
	}

	return (
		<Popover open={abierto} onOpenChange={setAbierto}>
			<WithTooltip content={etiqueta}>
				<PopoverTrigger asChild>{boton}</PopoverTrigger>
			</WithTooltip>
			<PopoverContent
				side="top"
				align="end"
				// Sin robar el foco al campo: así el cursor sigue donde estaba.
				onOpenAutoFocus={(e) => e.preventDefault()}
				onCloseAutoFocus={(e) => e.preventDefault()}
				// El chat devuelve el foco al campo tras cada emoji: eso no cierra el
				// selector (un clic fuera, sí), para poner varios seguidos.
				onFocusOutside={(e) => e.preventDefault()}
				className="w-auto overflow-hidden p-0"
			>
				<Suspense fallback={<Cargando />}>
					<SelectorEmoji alElegir={alElegir} />
				</Suspense>
			</PopoverContent>
		</Popover>
	);
}
