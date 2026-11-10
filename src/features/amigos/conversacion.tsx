import { useCallback, useEffect, useRef, useState } from "react";
import type { AmigoVista } from "@/bindings/AmigoVista";
import type { MensajeAmigo } from "@/bindings/MensajeAmigo";
import { BotonEmoji } from "@/components/emoji/boton-emoji";
import { insertarEnCursor } from "@/components/emoji/insertar";
import { BotonMenu, FilaMenu } from "@/components/hoja-menu";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { formatoActual, useFormato } from "@/lib/formato";
import { localeDe } from "@/lib/idiomas";
import { call, on } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { nombreAmigo, useAmigos, useEscribiendo } from "@/stores/amigos-store";
import { useRed } from "@/stores/red-store";
import { useUi } from "@/stores/ui-store";
import { NombreYNota } from "./nombres";
import { Denunciar, Seguridad } from "./seguridad";

/** Cada cuánto, como mucho, se avisa de que se está escribiendo (el otro lo enseña 6 s). */
const AVISO_ESCRIBIENDO_MS = 3000;

/** Lo más largo de un mensaje (Rust lo comprueba igual). */
export const MAX_MENSAJE = 2000;

/**
 * **La conversación con un amigo** (plan D4 y D5): los mensajes, el campo con el botón de emojis
 * y, si esta app tiene una partida en red abierta como anfitrión, «Invitar a una partida».
 */
export function Conversacion({
	amigo,
	onError,
}: {
	amigo: AmigoVista;
	onError: (e: unknown) => void;
}) {
	const { t, lang } = useLang();
	const [mensajes, setMensajes] = useState<MensajeAmigo[]>([]);
	const [texto, setTexto] = useState("");
	const [seguridad, setSeguridad] = useState(false);
	const [denuncia, setDenuncia] = useState(false);
	const [renombrar, setRenombrar] = useState(false);
	const escribe = useEscribiendo(amigo.clave);
	const nombre = nombreAmigo(amigo);
	const ultimoAviso = useRef(0);
	// «Estoy escribiendo», como mucho cada 3 s y solo con algo escrito.
	const avisarEscribiendo = (nuevo: string) => {
		const ahora = Date.now();
		if (!nuevo.trim() || ahora - ultimoAviso.current < AVISO_ESCRIBIENDO_MS) return;
		ultimoAviso.current = ahora;
		void call("amigos_escribiendo", { clave: amigo.clave }).catch(() => {});
	};
	const campo = useRef<HTMLInputElement>(null);
	const cursor = useRef<{ inicio: number | null; fin: number | null } | null>(null);
	const final = useRef<HTMLDivElement>(null);
	const codigoPartida = useRed((s) => (s.papel === "anfitrion" && !s.empezada ? s.codigo : null));
	const hora = new Intl.DateTimeFormat(localeDe(lang), {
		hour: "2-digit",
		minute: "2-digit",
	});

	const cargar = useCallback(async () => {
		const v = await call("amigos_conversacion", { clave: amigo.clave, antes: null });
		setMensajes(v);
		await call("amigos_leido", { clave: amigo.clave });
	}, [amigo.clave]);

	// Al abrir: la conversación, «leída», y el `sync` cada 10 s mientras se ve.
	useEffect(() => {
		void cargar().catch(onError);
		void call("amigos_ritmo", { chatAbierto: true });
		const dejar = on("amigos://mensajes", (claves) => {
			if (claves.includes(amigo.clave)) void cargar().catch(() => {});
		});
		return () => {
			void call("amigos_ritmo", { chatAbierto: false });
			void dejar.then((f) => f());
		};
	}, [amigo.clave, cargar, onError]);

	useEffect(() => {
		const f = requestAnimationFrame(() => final.current?.scrollIntoView({ block: "end" }));
		return () => cancelAnimationFrame(f);
	}, [mensajes]);

	const poner = (m: MensajeAmigo) =>
		setMensajes((v) => [...v.filter((x) => x.id !== m.id), m].sort((a, b) => a.orden - b.orden));

	const mandar = async () => {
		const limpio = texto.trim();
		if (!limpio) return;
		try {
			const r = await call("amigos_mandar", { clave: amigo.clave, texto: limpio });
			setTexto("");
			cursor.current = null;
			// Lo siguiente que se escriba vuelve a avisar enseguida.
			ultimoAviso.current = 0;
			poner(r.mensaje);
			if (r.error) onError(r.error);
		} catch (e) {
			onError(e);
		}
	};
	const reintentar = async (id: number) => {
		try {
			const r = await call("amigos_reintentar", { id });
			poner(r.mensaje);
			if (r.error) onError(r.error);
		} catch (e) {
			onError(e);
		}
	};
	const invitar = async () => {
		if (!codigoPartida) return;
		try {
			const r = await call("amigos_invitar", { clave: amigo.clave, codigo: codigoPartida });
			poner(r.mensaje);
			if (r.error) onError(r.error);
		} catch (e) {
			onError(e);
		}
	};
	const unirse = (codigo: string) => {
		useAmigos.getState().pedirUnirse(codigo);
		useAmigos.getState().cerrar();
		useUi.getState().setVista("mesa");
	};
	const ponerEmoji = (emoji: string) => {
		const el = campo.current;
		const conFoco = el !== null && document.activeElement === el;
		const sel = conFoco
			? { inicio: el.selectionStart, fin: el.selectionEnd }
			: (cursor.current ?? { inicio: null, fin: null });
		const r = insertarEnCursor(texto, sel.inicio, sel.fin, emoji, MAX_MENSAJE);
		if (!r) return;
		setTexto(r.texto);
		cursor.current = { inicio: r.cursor, fin: r.cursor };
		requestAnimationFrame(() => {
			const c = campo.current;
			if (!c) return;
			if (!formatoActual().tactil) c.focus();
			c.setSelectionRange(r.cursor, r.cursor);
		});
	};

	const quitar = () =>
		void call("amigos_quitar", { clave: amigo.clave })
			.then(() => useAmigos.getState().verChat(null))
			.catch(onError);
	const bloquear = () =>
		void call("amigos_bloquear", { clave: amigo.clave, bloquear: true })
			.then(() => useAmigos.getState().verChat(null))
			.catch(onError);

	return (
		<div className="flex min-h-0 flex-1 flex-col gap-2">
			{/* Sin la cabecera del diálogo (de lado y con el teclado), la X queda a esta altura. */}
			<div className="flex items-center gap-1.5 landscape:[[data-formato=movil][data-teclado]_&]:pr-8">
				<WithTooltip content={t("Volver a la lista")}>
					<Button
						size="icon-sm"
						variant="ghost"
						aria-label={t("Volver a la lista")}
						onClick={() => useAmigos.getState().verChat(null)}
					>
						<Icono icon="lucide:arrow-left" className="size-4" />
					</Button>
				</WithTooltip>
				<span
					className={cn(
						"size-2.5 shrink-0 rounded-full",
						amigo.enLinea ? "bg-emerald-500" : "bg-muted-foreground/40",
					)}
					role="img"
					aria-label={amigo.enLinea ? t("Conectado") : t("Desconectado")}
				/>
				<span className="flex min-w-0 flex-1 flex-col">
					<span className="truncate text-sm font-semibold">{nombre}</span>
					{escribe ? (
						<span className="truncate text-[11px] text-primary">{t("Escribiendo…")}</span>
					) : (
						amigo.alias && (
							<span className="truncate text-[11px] text-muted-foreground">{amigo.apodo}</span>
						)
					)}
				</span>
				<MenuAmigo
					titulo={nombre}
					acciones={[
						{
							icono: "lucide:pencil",
							texto: t("Nombre y nota"),
							onClick: () => setRenombrar(true),
						},
						{
							icono: amigo.verificado ? "lucide:shield-check" : "lucide:shield",
							texto: t("Número de seguridad"),
							onClick: () => setSeguridad(true),
						},
						{ icono: "lucide:user-minus", texto: t("Quitar de amigos"), onClick: quitar },
						{ icono: "lucide:ban", texto: t("Bloquear"), onClick: bloquear, destructiva: true },
						{
							icono: "lucide:flag",
							texto: t("Denunciar"),
							onClick: () => setDenuncia(true),
							destructiva: true,
						},
					]}
				/>
			</div>

			{amigo.nota && (
				<button
					type="button"
					onClick={() => setRenombrar(true)}
					className="flex items-start gap-2 rounded-lg border border-dashed p-2 text-left text-xs text-muted-foreground hover:bg-accent"
				>
					<Icono icon="lucide:sticky-note" className="mt-px size-4 shrink-0" />
					<span className="line-clamp-3 break-words whitespace-pre-wrap">{amigo.nota}</span>
				</button>
			)}
			{amigo.claveCambio && (
				<button
					type="button"
					onClick={() => setSeguridad(true)}
					className="flex items-center gap-2 rounded-lg border border-amber-500/60 p-2 text-left text-xs"
				>
					<Icono icon="lucide:shield-alert" className="size-4 shrink-0 text-amber-500" />
					{t(
						"La clave de {apodo} ha cambiado (¿ha reinstalado la app?). Comprueba el número de seguridad.",
						{ apodo: nombre },
					)}
				</button>
			)}
			{!amigo.claveValida && (
				<p className="rounded-lg border border-destructive p-2 text-xs text-destructive">
					{t("No se puede escribir a esta persona: su clave no está bien firmada.")}
				</p>
			)}

			<ScrollArea className="min-h-0 flex-1" viewportClassName="h-full">
				<div className="flex flex-col gap-1.5 p-1 text-xs">
					{mensajes.length === 0 && (
						<p className="p-4 text-center text-muted-foreground">{t("Todavía no hay mensajes.")}</p>
					)}
					{mensajes.map((m) => (
						<Burbuja
							key={m.id}
							m={m}
							hora={hora.format(new Date(m.orden))}
							onReintentar={() => void reintentar(m.id)}
							onUnirse={unirse}
						/>
					))}
					<div ref={final} />
				</div>
			</ScrollArea>

			<div className="flex flex-col gap-1.5">
				{codigoPartida && (
					<Button size="sm" variant="outline" onClick={() => void invitar()}>
						<Icono icon="lucide:swords" className="size-4" />
						{t("Invitar a tu partida")}
					</Button>
				)}
				<div className="flex gap-1.5">
					<Input
						ref={campo}
						value={texto}
						maxLength={MAX_MENSAJE}
						placeholder={t("Escribe un mensaje…")}
						aria-label={t("Escribe un mensaje…")}
						disabled={!amigo.claveValida}
						onChange={(e) => {
							setTexto(e.target.value);
							avisarEscribiendo(e.target.value);
						}}
						onBlur={(e) => {
							cursor.current = {
								inicio: e.currentTarget.selectionStart,
								fin: e.currentTarget.selectionEnd,
							};
						}}
						onKeyDown={(e) => {
							if (e.key === "Enter") {
								e.preventDefault();
								void mandar();
							}
						}}
					/>
					<BotonEmoji alElegir={ponerEmoji} disabled={!amigo.claveValida} />
					<Button
						size="icon"
						onClick={() => void mandar()}
						disabled={!texto.trim() || !amigo.claveValida}
						aria-label={t("Enviar")}
					>
						<Icono icon="lucide:send" className="size-4" />
					</Button>
				</div>
			</div>
			<Seguridad amigo={amigo} abierto={seguridad} onCerrar={() => setSeguridad(false)} />
			<NombreYNota
				amigo={amigo}
				abierto={renombrar}
				onCerrar={() => setRenombrar(false)}
				onError={onError}
			/>
			<Denunciar
				amigo={amigo}
				abierto={denuncia}
				onCerrar={() => setDenuncia(false)}
				onError={onError}
			/>
		</div>
	);
}

function Burbuja({
	m,
	hora,
	onReintentar,
	onUnirse,
}: {
	m: MensajeAmigo;
	hora: string;
	onReintentar: () => void;
	onUnirse: (codigo: string) => void;
}) {
	const { t } = useLang();
	const estado: Record<string, string> = {
		pendiente: t("Enviando…"),
		enviado: t("Enviado"),
		entregado: t("Entregado"),
		fallido: t("No se ha podido enviar"),
	};
	return (
		<div
			className={cn(
				"flex max-w-[85%] flex-col gap-0.5 rounded-xl px-2.5 py-1.5",
				m.mio ? "self-end bg-primary/15" : "self-start bg-muted/60",
			)}
		>
			{m.tipo === "invitacion" ? (
				<div className="flex flex-col gap-1.5">
					<span className="flex items-center gap-1.5 font-medium">
						<Icono icon="lucide:swords" className="size-4 text-primary" />
						{m.mio ? t("Has invitado a una partida") : t("Te invita a una partida")}
					</span>
					{!m.mio && (
						<Button size="sm" onClick={() => onUnirse(m.texto)}>
							{t("Unirse")}
						</Button>
					)}
				</div>
			) : (
				<span className="break-words whitespace-pre-wrap select-text">{m.texto}</span>
			)}
			<span className="flex items-center gap-1 self-end text-[10px] text-muted-foreground">
				<span className="tabular-nums">{hora}</span>
				{m.mio && <span>· {estado[m.estado] ?? m.estado}</span>}
				{m.mio && m.estado === "fallido" && (
					<Button size="xs" variant="link" className="h-auto p-0" onClick={onReintentar}>
						{t("Reintentar")}
					</Button>
				)}
			</span>
		</div>
	);
}

type AccionAmigo = {
	icono: string;
	texto: string;
	onClick: () => void;
	destructiva?: boolean;
};

/** Lo secundario de un amigo: un desplegable en escritorio y una hoja que sube en el móvil. */
function MenuAmigo({ titulo, acciones }: { titulo: string; acciones: AccionAmigo[] }) {
	const { t } = useLang();
	const movil = useFormato().tamano === "movil";
	if (movil) {
		return (
			<BotonMenu etiqueta={t("Más")} titulo={titulo}>
				{acciones.map((a) => (
					<FilaMenu
						key={a.icono}
						icono={a.icono}
						texto={a.texto}
						onClick={a.onClick}
						destructiva={a.destructiva}
					/>
				))}
			</BotonMenu>
		);
	}
	return (
		<DropdownMenu>
			<WithTooltip content={t("Más")}>
				<DropdownMenuTrigger asChild>
					<Button size="icon-sm" variant="ghost" aria-label={t("Más")}>
						<Icono icon="lucide:more-horizontal" className="size-4" />
					</Button>
				</DropdownMenuTrigger>
			</WithTooltip>
			<DropdownMenuContent align="end" className="w-52">
				{acciones.map((a) => (
					<DropdownMenuItem
						key={a.icono}
						variant={a.destructiva ? "destructive" : "default"}
						onSelect={a.onClick}
					>
						<Icono icon={a.icono} className="size-4" />
						{a.texto}
					</DropdownMenuItem>
				))}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
