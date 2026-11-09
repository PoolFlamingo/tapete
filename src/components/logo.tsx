import { Icono } from "@/components/icono";
import { cn } from "@/lib/utils";

/**
 * **El logo de Tapete**: la mano que juega una carta sobre el paño verde, el mismo
 * que el icono de la app (`src-tauri/icons/icono.svg`) y el de la web. Los colores son
 * de la marca y no siguen el tema: con cualquier tema es el mismo logo.
 */
export function Logo({ className }: { className?: string }) {
	return (
		<span
			aria-hidden
			className={cn(
				"grid size-7 shrink-0 place-items-center rounded-[28%] text-[#f0fdf4]",
				"bg-[radial-gradient(120%_90%_at_50%_0%,#1f7a45_0%,#14532d_55%,#0b3b1f_100%)]",
				className,
			)}
		>
			<Icono icon="game-icons:card-play" className="size-[62%]" />
		</span>
	);
}
