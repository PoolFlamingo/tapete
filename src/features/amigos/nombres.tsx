import { useEffect, useState } from "react";
import type { AmigoVista } from "@/bindings/AmigoVista";
import { DialogoAdaptable } from "@/components/dialogo-adaptable";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { call } from "@/lib/ipc";
import { useAmigos } from "@/stores/amigos-store";

/** Lo más largo de un nombre (Rust corta igual: `MAX_ALIAS`, `MAX_APODO`). */
const MAX_NOMBRE = 32;

/** Lo más larga que puede ser una nota (Rust corta igual: `MAX_NOTA`). */
const MAX_NOTA = 500;

/**
 * **Nombre y nota de un amigo**, solo para mí (`amigos_personal`): el nombre con el que lo veo y
 * una nota, como la de los amigos del LoL. Él no los ve y el servidor no los sabe. Vacío quita
 * cada uno (sin nombre vuelve a salir el suyo).
 */
export function NombreYNota({
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
	const [nombre, setNombre] = useState("");
	const [nota, setNota] = useState("");
	useEffect(() => {
		if (!abierto) return;
		setNombre(amigo.alias ?? "");
		setNota(amigo.nota ?? "");
	}, [abierto, amigo.alias, amigo.nota]);
	const guardar = async () => {
		try {
			useAmigos
				.getState()
				.poner(await call("amigos_personal", { clave: amigo.clave, alias: nombre, nota }));
			onCerrar();
		} catch (e) {
			onError(e);
		}
	};
	return (
		<Dialog open={abierto} onOpenChange={(o) => !o && onCerrar()}>
			<DialogoAdaptable compacto className="sm:max-w-sm">
				<DialogHeader>
					<DialogTitle>{t("Nombre y nota")}</DialogTitle>
					<DialogDescription>
						{t("Solo lo ves tú, en este equipo. {apodo} sigue viendo el suyo.", {
							apodo: amigo.apodo,
						})}
					</DialogDescription>
				</DialogHeader>
				<div className="flex flex-col gap-2.5 text-xs">
					<label className="flex flex-col gap-1">
						<span className="font-medium">{t("Nombre")}</span>
						<Input
							value={nombre}
							maxLength={MAX_NOMBRE}
							placeholder={amigo.apodo}
							autoComplete="off"
							onChange={(e) => setNombre(e.target.value)}
						/>
					</label>
					<label className="flex flex-col gap-1">
						<span className="font-medium">{t("Nota")}</span>
						<textarea
							value={nota}
							maxLength={MAX_NOTA}
							placeholder={t("Lo que quieras recordar: de qué le conoces, qué decks juega…")}
							onChange={(e) => setNota(e.target.value)}
							className="min-h-20 rounded-md border bg-transparent p-2 text-xs select-text"
						/>
					</label>
				</div>
				<DialogFooter>
					<Button size="sm" variant="outline" onClick={onCerrar}>
						{t("Cancelar")}
					</Button>
					<Button size="sm" onClick={() => void guardar()}>
						{t("Guardar")}
					</Button>
				</DialogFooter>
			</DialogoAdaptable>
		</Dialog>
	);
}

/**
 * **Mi nombre**: con el que me ven mis amigos y los equipos cerca al pasar decks (el mismo,
 * `pasar_nombre`). Vacío vuelve al del sistema. Al cambiarlo, el servidor se lo cuenta a mis
 * amigos al momento.
 */
export function TuNombre({ onError }: { onError: (e: unknown) => void }) {
	const { t } = useLang();
	const [nombre, setNombre] = useState<string | null>(null);
	const [editando, setEditando] = useState<string | null>(null);
	useEffect(() => {
		void call("pasar_nombre")
			.then(setNombre)
			.catch(() => {});
	}, []);
	const guardar = async () => {
		if (editando === null) return;
		try {
			setNombre(await call("pasar_nombre_set", { nombre: editando.trim() }));
			setEditando(null);
		} catch (e) {
			onError(e);
		}
	};
	return (
		<section className="flex flex-col gap-1.5 rounded-lg border p-2.5">
			<span className="font-medium">{t("Tu nombre")}</span>
			{editando === null ? (
				<div className="flex items-center gap-1.5">
					<span className="min-w-0 flex-1 truncate text-sm">{nombre ?? "…"}</span>
					<WithTooltip content={t("Cambiar tu nombre")}>
						<Button
							size="icon-sm"
							variant="outline"
							aria-label={t("Cambiar tu nombre")}
							disabled={nombre === null}
							onClick={() => setEditando(nombre ?? "")}
						>
							<Icono icon="lucide:pencil" className="size-4" />
						</Button>
					</WithTooltip>
				</div>
			) : (
				<div className="flex gap-1.5">
					<Input
						value={editando}
						maxLength={MAX_NOMBRE}
						aria-label={t("Tu nombre")}
						autoComplete="off"
						autoFocus
						onChange={(e) => setEditando(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Enter") void guardar();
							if (e.key === "Escape") {
								e.stopPropagation();
								setEditando(null);
							}
						}}
					/>
					<Button size="sm" onClick={() => void guardar()}>
						{t("Guardar")}
					</Button>
					<Button size="sm" variant="outline" onClick={() => setEditando(null)}>
						{t("Cancelar")}
					</Button>
				</div>
			)}
			<span className="text-muted-foreground">
				{t("Así te ven tus amigos y los equipos cerca al pasar decks. Vacío: el del dispositivo.")}
			</span>
		</section>
	);
}
