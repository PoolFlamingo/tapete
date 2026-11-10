import { useState } from "react";
import type { AmigoVista } from "@/bindings/AmigoVista";
import { DialogoAdaptable } from "@/components/dialogo-adaptable";
import { Icono } from "@/components/icono";
import { paraTraducir, useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { call } from "@/lib/ipc";

/**
 * **El número de seguridad** (plan D3): 12 cifras sacadas de las claves de los dos, iguales en
 * los dos equipos. Si coinciden al compararlas en persona (o por otro canal de confianza), nadie
 * se ha metido en medio: el amigo queda «verificado». Si su clave cambia, la app avisa.
 */
export function Seguridad({
	amigo,
	abierto,
	onCerrar,
}: {
	amigo: AmigoVista;
	abierto: boolean;
	onCerrar: () => void;
}) {
	const { t } = useLang();
	const marcar = (verificado: boolean) => {
		void call("amigos_verificar", { clave: amigo.clave, verificado });
		onCerrar();
	};
	return (
		<Dialog open={abierto} onOpenChange={(o) => !o && onCerrar()}>
			<DialogoAdaptable compacto className="sm:max-w-sm" cambiosSinGuardar={false}>
				<DialogHeader>
					<DialogTitle>{t("Número de seguridad")}</DialogTitle>
					<DialogDescription>
						{t(
							"Comparadlo en persona o por otro medio de confianza. Si es el mismo en los dos equipos, nadie lee vuestros mensajes por el camino.",
						)}
					</DialogDescription>
				</DialogHeader>
				<div className="rounded-lg border p-3 text-center font-mono text-xl tracking-widest select-text">
					{amigo.numeroSeguridad}
				</div>
				{amigo.verificado && (
					<p className="flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
						<Icono icon="lucide:shield-check" className="size-4" />
						{t("Ya lo comprobaste.")}
					</p>
				)}
				<DialogFooter>
					{amigo.verificado || amigo.claveCambio ? (
						<Button size="sm" variant="outline" onClick={() => marcar(false)}>
							{t("Quitar la marca")}
						</Button>
					) : (
						<Button size="sm" variant="outline" onClick={onCerrar}>
							{t("Ahora no")}
						</Button>
					)}
					<Button size="sm" onClick={() => marcar(true)}>
						{t("Coincide")}
					</Button>
				</DialogFooter>
			</DialogoAdaptable>
		</Dialog>
	);
}

const MOTIVOS = [
	["spam", paraTraducir("Mensajes no deseados")],
	["abuse", paraTraducir("Insultos o acoso")],
	["impersonation", paraTraducir("Se hace pasar por otra persona")],
	["cheating", paraTraducir("Hace trampas")],
	["other", paraTraducir("Otro motivo")],
] as const;

/** Denunciar a alguien: motivo y, si se quiere, una nota (nunca se mandan los mensajes). */
export function Denunciar({
	amigo,
	abierto,
	onCerrar,
	onError,
}: {
	amigo: AmigoVista;
	abierto: boolean;
	onCerrar: () => void;
	onError: (e: unknown) => void;
}) {
	const { t } = useLang();
	const [motivo, setMotivo] = useState<string>("abuse");
	const [nota, setNota] = useState("");
	const [ocupado, setOcupado] = useState(false);
	const mandar = async () => {
		setOcupado(true);
		try {
			await call("amigos_denunciar", { clave: amigo.clave, motivo, nota: nota.trim() || null });
			onCerrar();
		} catch (e) {
			onError(e);
		} finally {
			setOcupado(false);
		}
	};
	return (
		<Dialog open={abierto} onOpenChange={(o) => !o && onCerrar()}>
			<DialogoAdaptable compacto className="sm:max-w-sm">
				<DialogHeader>
					<DialogTitle>{t("Denunciar a {apodo}", { apodo: amigo.apodo })}</DialogTitle>
					<DialogDescription>
						{t(
							"Lo revisa quien lleva el servidor. Tus mensajes no se mandan: si quieres enseñar uno, pégalo en la nota.",
						)}
					</DialogDescription>
				</DialogHeader>
				<div className="flex flex-col gap-1 text-xs" role="radiogroup">
					{MOTIVOS.map(([id, texto]) => (
						<label key={id} className="flex min-h-9 items-center gap-2">
							<input
								type="radio"
								name="motivo"
								value={id}
								checked={motivo === id}
								onChange={() => setMotivo(id)}
							/>
							{t(texto)}
						</label>
					))}
					<textarea
						value={nota}
						maxLength={2000}
						onChange={(e) => setNota(e.target.value)}
						placeholder={t("Nota (opcional)")}
						aria-label={t("Nota (opcional)")}
						className="mt-1 min-h-20 rounded-md border bg-transparent p-2 text-xs select-text"
					/>
				</div>
				<DialogFooter>
					<Button size="sm" variant="outline" onClick={onCerrar}>
						{t("Cancelar")}
					</Button>
					<Button size="sm" variant="destructive" disabled={ocupado} onClick={() => void mandar()}>
						{t("Denunciar")}
					</Button>
				</DialogFooter>
			</DialogoAdaptable>
		</Dialog>
	);
}
