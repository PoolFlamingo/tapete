import { useEffect, useState } from "react";
import { toast } from "sonner";
import type { AmigoVista } from "@/bindings/AmigoVista";
import type { EstadoAmigos } from "@/bindings/EstadoAmigos";
import type { SolicitudVista } from "@/bindings/SolicitudVista";
import { Confirmar } from "@/components/confirmar";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { compartirTexto, copiarTexto, usePuedeCompartir } from "@/features/mesa/compartir";
import { urlSvg } from "@/features/pasar/recibir";
import { abrirAjustesDeLaCamara, escanearQr, escanerDisponible } from "@/lib/escaner";
import { localeDe } from "@/lib/idiomas";
import { call } from "@/lib/ipc";
import { useOcupado } from "@/lib/use-ocupado";
import { cn } from "@/lib/utils";
import { nombreAmigo, useAmigos, useEscribiendo } from "@/stores/amigos-store";
import { TuNombre } from "./nombres";
import { vistoHace } from "./visto";

/** La lista: mi código, añadir, solicitudes, amigos y salir. */
export function Lista({
	estado,
	onError,
	onAviso,
}: {
	estado: EstadoAmigos;
	onError: (e: unknown) => void;
	onAviso: (texto: string) => void;
}) {
	const { t, lang } = useLang();
	// Con un enlace de amigo, su código ya puesto (pedir lo pulsa la persona).
	const [codigo, setCodigo] = useState("");
	const anadir = useAmigos((s) => s.anadir);
	// También si el enlace llega con la lista ya abierta.
	useEffect(() => {
		if (!anadir) return;
		setCodigo(anadir);
		useAmigos.getState().pedirAnadir(null);
	}, [anadir]);
	const puedeCompartir = usePuedeCompartir();
	const [qr, setQr] = useState<string | null>(null);
	const [hayEscaner, setHayEscaner] = useState(false);
	useEffect(() => {
		void escanerDisponible().then(setHayEscaner);
	}, []);
	// El QR se pide al enseñarlo, y otra vez si el código cambia.
	const verQr = qr !== null;
	useEffect(() => {
		if (verQr && estado.codigo)
			void call("amigos_qr")
				.then((svg) => setQr(svg ?? ""))
				.catch(onError);
	}, [verQr, estado.codigo, onError]);
	const [ocupado, envolver] = useOcupado();
	const [salir, setSalir] = useState(false);

	// Un doble clic no lo manda dos veces.
	const hacer = (f: () => Promise<unknown>) => envolver(f).catch(onError);
	const pedir = (texto = codigo) =>
		hacer(async () => {
			const r = await call("amigos_pedir", { codigo: texto.trim() });
			setCodigo("");
			onAviso(
				r === "accepted"
					? t("Ya sois amigos: esa persona también te lo había pedido.")
					: t("Solicitud enviada. Cuando la acepte, saldrá en tu lista."),
			);
		});
	const copiarCodigo = async () => {
		if (estado.codigo && (await copiarTexto(estado.codigo))) onAviso(t("Código copiado"));
	};
	// El enlace de la web: abre la app con el código puesto o lleva a descargarla.
	const enlace = async () =>
		estado.codigo ? await call("enlace_amigo", { codigo: estado.codigo }).catch(() => null) : null;
	const pasarEnlace = async () => {
		const e = await enlace();
		if (!e) return;
		if (puedeCompartir) await compartirTexto(e);
		else if (await copiarTexto(e)) onAviso(t("Enlace copiado"));
	};

	// El QR de otro: su enlace de amigo (o el código suelto); Rust saca el código.
	const escanear = async () => {
		const r = await escanearQr();
		if (r.tipo === "leido") {
			setCodigo(r.texto);
			await pedir(r.texto);
		} else if (r.tipo === "sin-permiso") {
			toast.error(t("Sin permiso para usar la cámara."), {
				action: { label: t("Ajustes"), onClick: () => void abrirAjustesDeLaCamara() },
			});
		}
	};

	return (
		<ScrollArea className="min-h-0 flex-1" viewportClassName="h-full">
			<div className="flex flex-col gap-2.5 p-1 text-xs">
				{estado.avisos.map((a) => (
					<div
						key={a.id}
						className={cn(
							"rounded-lg border p-2.5",
							a.nivel === "critical" && "border-destructive text-destructive",
							a.nivel === "warning" && "border-amber-500/60",
						)}
					>
						{a.textos[lang] ?? a.textos.en ?? a.textos.es ?? ""}
					</div>
				))}

				<TuNombre onError={onError} />

				<section className="flex flex-col gap-1.5 rounded-lg border p-2.5">
					<span className="font-medium">{t("Tu código de amigo")}</span>
					{/* El código entero en una línea; los botones bajan si no caben (móvil). */}
					<div className="flex flex-wrap items-center gap-1.5">
						<span className="flex-1 font-mono text-base tracking-wider whitespace-nowrap select-text">
							{estado.codigo ?? "…"}
						</span>
						<div className="ml-auto flex gap-1.5">
							<WithTooltip content={t("Copiar")}>
								<Button
									size="icon-sm"
									variant="outline"
									aria-label={t("Copiar")}
									disabled={!estado.codigo}
									onClick={() => void copiarCodigo()}
								>
									<Icono icon="lucide:copy" className="size-4" />
								</Button>
							</WithTooltip>
							<WithTooltip content={puedeCompartir ? t("Compartir") : t("Copiar enlace")}>
								<Button
									size="icon-sm"
									variant="outline"
									aria-label={puedeCompartir ? t("Compartir") : t("Copiar enlace")}
									disabled={!estado.codigo}
									onClick={() => void pasarEnlace()}
								>
									<Icono
										icon={puedeCompartir ? "lucide:share-2" : "lucide:link"}
										className="size-4"
									/>
								</Button>
							</WithTooltip>
							<WithTooltip content={verQr ? t("Ocultar el QR") : t("Enseñar el QR")}>
								<Button
									size="icon-sm"
									variant={verQr ? "secondary" : "outline"}
									aria-label={verQr ? t("Ocultar el QR") : t("Enseñar el QR")}
									aria-pressed={verQr}
									disabled={!estado.codigo}
									onClick={() => setQr(verQr ? null : "")}
								>
									<Icono icon="lucide:qr-code" className="size-4" />
								</Button>
							</WithTooltip>
							<WithTooltip content={t("Cambiar el código (el viejo deja de valer)")}>
								<Button
									size="icon-sm"
									variant="outline"
									aria-label={t("Cambiar el código (el viejo deja de valer)")}
									disabled={ocupado}
									onClick={() => void hacer(() => call("amigos_nuevo_codigo"))}
								>
									<Icono icon="lucide:refresh-cw" className="size-4" />
								</Button>
							</WithTooltip>
						</div>
					</div>
					{qr && (
						<img
							src={urlSvg(qr)}
							alt={t("Código QR de tu código de amigo")}
							className="mx-auto size-48 max-w-full rounded-xl bg-white p-1"
						/>
					)}
					<span className="text-muted-foreground">
						{t("Pásaselo a quien quieras añadir; con él te pide amistad.")}
					</span>
				</section>

				<section className="flex flex-col gap-1.5 rounded-lg border p-2.5">
					<label htmlFor="codigo-amigo" className="font-medium">
						{t("Añadir con un código")}
					</label>
					<div className="flex gap-1.5">
						<Input
							id="codigo-amigo"
							value={codigo}
							placeholder="TAP-XXXX-XXXX"
							autoComplete="off"
							spellCheck={false}
							onChange={(e) => setCodigo(e.target.value)}
							onKeyDown={(e) => {
								if (e.key === "Enter" && codigo.trim()) void pedir();
							}}
						/>
						<Button size="sm" disabled={!codigo.trim() || ocupado} onClick={() => void pedir()}>
							{t("Pedir")}
						</Button>
						{hayEscaner && (
							<WithTooltip content={t("Escanear un QR")}>
								<Button
									size="icon-sm"
									variant="outline"
									aria-label={t("Escanear un QR")}
									disabled={ocupado}
									onClick={() => void escanear()}
								>
									<Icono icon="lucide:scan-line" className="size-4" />
								</Button>
							</WithTooltip>
						)}
					</div>
				</section>

				{estado.recibidas.length > 0 && (
					<section className="flex flex-col gap-1.5">
						<h3 className="px-1 font-medium">{t("Te piden amistad")}</h3>
						{estado.recibidas.map((s) => (
							<FilaSolicitud key={s.id} s={s}>
								<Button
									size="sm"
									disabled={ocupado}
									onClick={() =>
										void hacer(() => call("amigos_responder", { id: s.id, aceptar: true }))
									}
								>
									{t("Aceptar solicitud")}
								</Button>
								<Button
									size="sm"
									variant="outline"
									disabled={ocupado}
									onClick={() =>
										void hacer(() => call("amigos_responder", { id: s.id, aceptar: false }))
									}
								>
									{t("Rechazar")}
								</Button>
							</FilaSolicitud>
						))}
					</section>
				)}

				<section className="flex flex-col gap-1.5">
					<h3 className="px-1 font-medium">{t("Amigos")}</h3>
					{estado.amigos.length === 0 && (
						<p className="px-1 text-muted-foreground">
							{t("Aún no tienes amigos aquí. Pasa tu código o pide el de alguien.")}
						</p>
					)}
					{estado.amigos.map((a) => (
						<FilaAmigo key={a.clave} a={a} />
					))}
				</section>

				{estado.enviadas.length > 0 && (
					<section className="flex flex-col gap-1.5">
						<h3 className="px-1 font-medium">{t("Esperando respuesta")}</h3>
						{estado.enviadas.map((s) => (
							<FilaSolicitud key={s.id} s={s}>
								<Button
									size="sm"
									variant="outline"
									disabled={ocupado}
									onClick={() =>
										void hacer(() => call("amigos_responder", { id: s.id, aceptar: false }))
									}
								>
									{t("Retirar")}
								</Button>
							</FilaSolicitud>
						))}
					</section>
				)}

				<div className="flex justify-end pt-1">
					<Button size="sm" variant="ghost" onClick={() => setSalir(true)}>
						<Icono icon="lucide:log-out" className="size-4" />
						{t("Dejar de usar amigos")}
					</Button>
				</div>
			</div>
			<Salir abierto={salir} onCerrar={() => setSalir(false)} onError={onError} />
		</ScrollArea>
	);
}

function FilaSolicitud({ s, children }: { s: SolicitudVista; children: React.ReactNode }) {
	return (
		<div className="flex items-center gap-2 rounded-lg border p-2">
			<Icono icon="lucide:user-round" className="size-4 shrink-0 text-muted-foreground" />
			<span className="min-w-0 flex-1 truncate font-medium">{s.apodo}</span>
			{children}
		</div>
	);
}

function FilaAmigo({ a }: { a: AmigoVista }) {
	const { t, lang } = useLang();
	const visto = a.enLinea ? null : vistoHace(a.visto, localeDe(lang));
	const escribe = useEscribiendo(a.clave);
	// Debajo del nombre: «escribiendo…»; si no, su nombre (si le puse otro) y cuándo se le vio.
	const debajo = [a.alias ? a.apodo : null, visto ? t("Visto {cuando}", { cuando: visto }) : null]
		.filter(Boolean)
		.join(" · ");
	return (
		<button
			type="button"
			onClick={() => useAmigos.getState().verChat(a.clave)}
			className="flex min-h-11 items-center gap-2 rounded-lg border p-2 text-left hover:bg-accent active:bg-accent"
		>
			<span
				className={cn(
					"size-2.5 shrink-0 rounded-full",
					a.enLinea ? "bg-emerald-500" : "bg-muted-foreground/40",
				)}
				role="img"
				aria-label={a.enLinea ? t("Conectado") : t("Desconectado")}
			/>
			<span className="flex min-w-0 flex-1 flex-col">
				<span className="truncate font-medium">{nombreAmigo(a)}</span>
				{escribe ? (
					<span className="truncate text-[11px] text-primary">{t("Escribiendo…")}</span>
				) : (
					debajo && <span className="truncate text-[11px] text-muted-foreground">{debajo}</span>
				)}
			</span>
			{a.nota && (
				<Icono
					icon="lucide:sticky-note"
					className="size-4 shrink-0 text-muted-foreground"
					aria-label={t("Tiene una nota")}
				/>
			)}
			{a.claveCambio && (
				<Icono
					icon="lucide:shield-alert"
					className="size-4 shrink-0 text-amber-500"
					aria-label={t("Su clave ha cambiado")}
				/>
			)}
			{a.verificado && !a.claveCambio && (
				<Icono
					icon="lucide:shield-check"
					className="size-4 shrink-0 text-emerald-500"
					aria-label={t("Verificado")}
				/>
			)}
			{a.sinLeer > 0 && (
				<span className="rounded-full bg-primary px-1.5 text-[10px] font-semibold text-primary-foreground tabular-nums">
					{a.sinLeer}
				</span>
			)}
			<Icono icon="lucide:chevron-right" className="size-4 shrink-0 text-muted-foreground" />
		</button>
	);
}

/** Dejar de usar el servidor; si se quiere, que borre esta clave y lo de aquí. */
function Salir({
	abierto,
	onCerrar,
	onError,
}: {
	abierto: boolean;
	onCerrar: () => void;
	onError: (e: unknown) => void;
}) {
	const { t } = useLang();
	const [borrarServidor, setBorrarServidor] = useState(true);
	const [borrarLocal, setBorrarLocal] = useState(false);
	return (
		<Confirmar
			abierto={abierto}
			onCerrar={onCerrar}
			titulo={t("¿Dejar de usar amigos?")}
			descripcion={t(
				"La app deja de llamar al servidor de Tapete. El resto de la app sigue igual, y puedes volver cuando quieras.",
			)}
			accion={t("Dejar de usarlo")}
			destructiva
			onConfirmar={() =>
				void call("amigos_desactivar", { borrarServidor, borrarLocal })
					.then((e) => useAmigos.getState().poner(e))
					.catch(onError)
			}
		>
			<div className="flex flex-col gap-2 text-xs">
				<label className="flex items-center justify-between gap-2.5">
					<span>{t("Borrar del servidor mi clave, mis amistades y mis mensajes")}</span>
					<Switch checked={borrarServidor} onCheckedChange={setBorrarServidor} />
				</label>
				<label className="flex items-center justify-between gap-2.5">
					<span>{t("Borrar también las conversaciones de este equipo")}</span>
					<Switch checked={borrarLocal} onCheckedChange={setBorrarLocal} />
				</label>
			</div>
		</Confirmar>
	);
}
