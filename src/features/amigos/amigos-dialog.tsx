import { useCallback, useEffect } from "react";
import { toast } from "sonner";
import { DialogoAdaptable } from "@/components/dialogo-adaptable";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { call } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useAmigos } from "@/stores/amigos-store";
import { Consentimiento } from "./consentimiento";
import { Conversacion } from "./conversacion";
import { Lista } from "./lista";

/**
 * **Amigos** (plan Tapete, Fase D): sin aceptar, la pantalla del consentimiento (D0); después, la
 * lista (mi código, añadir, solicitudes, amigos) o la conversación con uno. A pantalla completa en
 * el móvil.
 */
export function AmigosDialog() {
	const { t, mensaje } = useLang();
	const abierto = useAmigos((s) => s.abierto);
	const chat = useAmigos((s) => s.chat);
	const estado = useAmigos((s) => s.estado);
	const amigo = chat ? (estado?.amigos.find((a) => a.clave === chat) ?? null) : null;
	const activo = estado?.activo ?? false;
	const onError = useCallback((e: unknown) => toast.error(mensaje(e)), [mensaje]);
	const onAviso = useCallback((texto: string) => toast.success(texto), []);

	// Al abrir: ponerse al día (lo que llegó mientras no se miraba).
	useEffect(() => {
		if (abierto && activo) void call("amigos_sincronizar").catch(() => {});
	}, [abierto, activo]);

	return (
		<Dialog open={abierto} onOpenChange={(o) => !o && useAmigos.getState().cerrar()}>
			<DialogoAdaptable
				// Sin aceptar, a la medida del texto; la lista y el chat, con alto fijo (no salta al
				// cambiar de una a otro).
				className={cn(
					"flex flex-col sm:max-w-lg",
					activo ? "h-[min(42rem,88dvh)]" : "max-h-[88dvh]",
				)}
				cambiosSinGuardar={false}
			>
				{/* De lado y con el teclado solo quedan ~150 dp: lo que queda es para el chat. */}
				<DialogHeader className="landscape:[[data-formato=movil][data-teclado]_&]:hidden">
					<DialogTitle className="flex items-center gap-1.5">
						<Icono icon="lucide:users" className="size-4 text-primary" />
						{t("Amigos")}
					</DialogTitle>
					<DialogDescription className={activo ? "sr-only" : undefined}>
						{t("Amigos, mensajes cifrados e invitaciones a partidas.")}
					</DialogDescription>
				</DialogHeader>
				{activo && estado?.error && (
					<div className="flex items-center gap-2 rounded-lg border border-amber-500/60 p-2 text-xs">
						<Icono icon="lucide:wifi-off" className="size-4 shrink-0 text-amber-500" />
						<span className="min-w-0 flex-1">{mensaje(estado.error)}</span>
						<Button
							size="xs"
							variant="outline"
							onClick={() => void call("amigos_sincronizar").catch(onError)}
						>
							{t("Reintentar")}
						</Button>
					</div>
				)}
				{!estado || !activo ? (
					<Consentimiento onError={onError} />
				) : amigo ? (
					<Conversacion key={amigo.clave} amigo={amigo} onError={onError} />
				) : (
					<Lista estado={estado} onError={onError} onAviso={onAviso} />
				)}
			</DialogoAdaptable>
		</Dialog>
	);
}

/**
 * Pasa a Rust los textos de los avisos del sistema, ya traducidos (Rust no escribe prosa), al
 * arrancar y al cambiar de idioma.
 */
export function TextosAvisosAmigos() {
	const { t } = useLang();
	useEffect(() => {
		void call("amigos_textos", {
			textos: {
				titulo: "Tapete",
				mensajeDe: t("Mensaje de {apodo}", { apodo: "{apodo}" }),
				invitacionDe: t("{apodo} te invita a una partida", { apodo: "{apodo}" }),
			},
		}).catch(() => {});
	}, [t]);
	return null;
}
