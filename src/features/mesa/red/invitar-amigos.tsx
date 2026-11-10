import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { call } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { nombreAmigo, useAmigos } from "@/stores/amigos-store";

/**
 * **Invitar amigos a la partida** desde la sala de espera, como en Steam: cada amigo con su botón,
 * los conectados primero. La invitación es un mensaje cifrado con el código sin IP
 * (`amigos_invitar`, `red::codigo::solo_rele`); el amigo pulsa «Unirse» y entra con su deck.
 * Si se llegó aquí con «Invitar a una partida» desde Amigos (`invitarA`), esa invitación sale sola.
 */
export function InvitarAmigos({ codigo }: { codigo: string }) {
	const { t, lang, mensaje } = useLang();
	const estado = useAmigos((s) => s.estado);
	const invitarA = useAmigos((s) => s.invitarA);
	const [invitados, setInvitados] = useState<Set<string>>(() => new Set());
	const [mandando, setMandando] = useState<string | null>(null);

	const invitar = async (clave: string) => {
		setMandando(clave);
		try {
			const r = await call("amigos_invitar", { clave, codigo });
			if (r.error) throw r.error;
			setInvitados((s) => new Set(s).add(clave));
		} catch (e) {
			toast.error(mensaje(e));
		} finally {
			setMandando(null);
		}
	};

	// La que se pidió desde Amigos, en cuanto hay código. Se lee y se vacía del store a la vez (no de
	// lo que vio este render): un efecto que corre dos veces no la manda dos veces.
	useEffect(() => {
		const clave = useAmigos.getState().invitarA;
		if (!clave) return;
		useAmigos.getState().pedirInvitar(null);
		void invitar(clave);
		// biome-ignore lint/correctness/useExhaustiveDependencies: una vez por petición.
	}, [invitarA]);

	if (!estado?.activo) return null;
	const amigos = estado.amigos
		.filter((a) => a.claveValida)
		.sort(
			(a, b) =>
				Number(b.enLinea) - Number(a.enLinea) || nombreAmigo(a).localeCompare(nombreAmigo(b), lang),
		);
	if (amigos.length === 0) return null;

	return (
		<div className="flex flex-col gap-1.5">
			<span className="text-xs font-medium">{t("Invitar amigos")}</span>
			<div className="flex flex-col gap-1">
				{amigos.map((a) => {
					const hecho = invitados.has(a.clave);
					return (
						<div
							key={a.clave}
							className="flex min-h-9 items-center gap-2 rounded-lg border px-2 py-1"
						>
							<span
								className={cn(
									"size-2 shrink-0 rounded-full",
									a.enLinea ? "bg-emerald-500" : "bg-muted-foreground/40",
								)}
								role="img"
								aria-label={a.enLinea ? t("Conectado") : t("Desconectado")}
							/>
							<span className="min-w-0 flex-1 truncate text-xs">{nombreAmigo(a)}</span>
							<Button
								size="sm"
								variant={hecho ? "ghost" : "outline"}
								disabled={mandando !== null}
								onClick={() => void invitar(a.clave)}
							>
								{mandando === a.clave ? (
									<Icono icon="lucide:loader-2" className="size-3.5 animate-spin" />
								) : (
									<Icono icon={hecho ? "lucide:check" : "lucide:send"} className="size-3.5" />
								)}
								{hecho ? t("Invitado") : t("Invitar")}
							</Button>
						</div>
					);
				})}
			</div>
		</div>
	);
}
