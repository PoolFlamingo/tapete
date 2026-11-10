import { Icono } from "@/components/icono";
import { cn } from "@/lib/utils";

/**
 * **El logo de Tapete**: la mano que juega una carta sobre el paño, como el icono de la
 * app (`src-tauri/icons/icono.svg`) y el de la web. En pantalla el paño toma el color
 * primario del tema elegido (Pablo, 10/11/2026): con el tema verde de fábrica es el verde
 * del icono y con uno naranja, naranja, para que el logo no desentone con el resto. El
 * degradado oscurece el primario hacia abajo, como el del icono, y la carta es casi
 * blanca con un toque del primario. El icono de la app instalada (el de la tienda, el
 * del escritorio) es una imagen y se queda verde.
 */
export function Logo({ className }: { className?: string }) {
	return (
		<span
			aria-hidden
			className={cn(
				"grid size-7 shrink-0 place-items-center rounded-[28%]",
				"text-[color-mix(in_oklab,var(--primary)_8%,white)]",
				"bg-[radial-gradient(120%_90%_at_50%_0%,color-mix(in_oklab,var(--primary)_80%,black)_0%,color-mix(in_oklab,var(--primary)_55%,black)_55%,color-mix(in_oklab,var(--primary)_35%,black)_100%)]",
				className,
			)}
		>
			<Icono icon="game-icons:card-play" className="size-[62%]" />
		</span>
	);
}
