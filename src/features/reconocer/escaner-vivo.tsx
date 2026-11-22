/**
 * **El escáner en vivo** (Android), como ManaBox: la cámara a pantalla
 * completa, un marco guía y el contorno de la carta que ve Rust; cada carta
 * nueva entra en la bandeja sin pulsar nada (`reconocer::vivo`).
 *
 * La cámara la pone Kotlin DETRÁS de la vista web (`EscanerCartas.kt`), así
 * que mientras está abierto `html[data-escaner]` deja transparentes `html` y
 * `body` y oculta la app (`#root`, sin desmontarla: `index.css`). Esto va en
 * un portal fuera de `#root`; los diálogos (la ficha de una carta) salen por
 * encima.
 */
import { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { EstadoEscaner } from "@/bindings/EstadoEscaner";
import type { Punto } from "@/bindings/Punto";
import { BarraSuperior } from "@/components/barra-superior";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { registrarAtras } from "@/lib/atras";
import { abrirAjustesDeLaCamara } from "@/lib/escaner";
import { call } from "@/lib/ipc";
import { useReconocer } from "@/stores/reconocer-store";
import { anadirAlDeck } from "./anadir";
import { Bandeja } from "./bandeja";
import { total } from "./cuentas";
import { elegirFotos } from "./reconocer-dialog";
import { encajar, marcoGuia } from "./visor";

export function EscanerVivo() {
	const vivo = useReconocer((s) => s.vivo);
	if (!vivo) return null;
	return createPortal(<Contenido />, document.body);
}

type Viendo = Extract<EstadoEscaner, { tipo: "viendo" }>;

/** Sale del escáner: con algo en la bandeja, al diálogo (no se pierde); si no, cierra. */
function salir() {
	const s = useReconocer.getState();
	if (s.items.length > 0) s.verVivo(false);
	else s.cerrar();
}

function Contenido() {
	const { t, mensaje } = useLang();
	const items = useReconocer((s) => s.items);
	const modo = useReconocer((s) => s.modo);
	const [abierta, setAbierta] = useState(false);
	const [error, setError] = useState<unknown>(null);
	const [viendo, setViendo] = useState<Viendo | null>(null);
	const [linterna, setLinterna] = useState(false);
	const [ultima, setUltima] = useState<string | null>(null);
	const [aviso, setAviso] = useState<string | null>(null);
	const [anadiendo, setAnadiendo] = useState(false);
	const [pantalla, setPantalla] = useState({ w: window.innerWidth, h: window.innerHeight });
	const mensajeRef = useRef(mensaje);
	mensajeRef.current = mensaje;

	// La cámara: se abre al montar y se cierra al desmontar.
	useEffect(() => {
		const html = document.documentElement;
		html.dataset.escaner = "";
		let vivo = true;
		const canal = new Channel<EstadoEscaner>();
		canal.onmessage = (m) => {
			if (!vivo) return;
			if (m.tipo === "viendo") {
				setViendo(m);
			} else if (m.tipo === "reconocida") {
				useReconocer.getState().recibir(m.reconocida);
				const c = m.reconocida.impresiones.find((i) => i.carta.id === m.reconocida.mejor)?.carta;
				setUltima(c ? (c.printedName ?? c.name) : null);
				setAviso(null);
				try {
					navigator.vibrate?.(40);
				} catch {
					// Sin vibración.
				}
			} else {
				setAviso(mensajeRef.current(m.error));
			}
		};
		call("reconocer_vivo_abrir", { canal })
			.then(() => vivo && setAbierta(true))
			.catch((e) => vivo && setError(e));
		return () => {
			vivo = false;
			delete html.dataset.escaner;
			void call("reconocer_vivo_cerrar", {}).catch(() => {});
		};
	}, []);

	// «Atrás» de Android sale del escáner.
	useEffect(
		() =>
			registrarAtras(() => {
				salir();
				return true;
			}),
		[],
	);

	useEffect(() => {
		const medir = () => setPantalla({ w: window.innerWidth, h: window.innerHeight });
		window.addEventListener("resize", medir);
		return () => window.removeEventListener("resize", medir);
	}, []);

	// El aviso de la última carta se va solo.
	useEffect(() => {
		if (!ultima) return;
		const id = setTimeout(() => setUltima(null), 2500);
		return () => clearTimeout(id);
	}, [ultima]);

	const cambiarLinterna = async () => {
		try {
			setLinterna(await call("reconocer_vivo_linterna", { si: !linterna }));
		} catch (e) {
			setAviso(mensaje(e));
		}
	};

	const anadir = async () => {
		setAnadiendo(true);
		try {
			if ((await anadirAlDeck(useReconocer.getState().items, modo, t)) > 0) {
				useReconocer.getState().cerrar();
			}
		} catch (e) {
			setAviso(mensaje(e));
		} finally {
			setAnadiendo(false);
		}
	};

	const n = total(items);
	const estado = error
		? null
		: !abierta
			? t("Abriendo la cámara…")
			: ultima
				? t("Reconocida: {carta}", { carta: ultima })
				: viendo?.leyendo
					? t("Leyendo…")
					: viendo?.esquinas
						? t("Quieto un momento…")
						: t("Apunta a una carta, de frente y con luz.");

	return (
		<div
			className="fixed inset-0 z-40 flex flex-col text-foreground select-none"
			data-escaner-vista=""
		>
			<BarraSuperior
				titulo={t("Escanear cartas")}
				inicio={
					<WithTooltip content={t("Cerrar")}>
						<Button size="icon-sm" variant="ghost" aria-label={t("Cerrar")} onClick={salir}>
							<Icono icon="lucide:x" />
						</Button>
					</WithTooltip>
				}
			>
				<WithTooltip content={linterna ? t("Apagar la linterna") : t("Encender la linterna")}>
					<Button
						size="icon-sm"
						variant={linterna ? "default" : "ghost"}
						aria-label={linterna ? t("Apagar la linterna") : t("Encender la linterna")}
						onClick={() => void cambiarLinterna()}
						disabled={!abierta}
					>
						<Icono icon={linterna ? "lucide:flashlight-off" : "lucide:flashlight"} />
					</Button>
				</WithTooltip>
				<WithTooltip content={t("Elegir de la galería")}>
					<Button
						size="icon-sm"
						variant="ghost"
						aria-label={t("Elegir de la galería")}
						onClick={() => void elegirFotos(t("Imágenes")).catch((e) => setAviso(mensaje(e)))}
					>
						<Icono icon="lucide:images" />
					</Button>
				</WithTooltip>
			</BarraSuperior>

			<div className="flex min-h-0 flex-1 flex-col landscape:flex-row">
				<div className="relative min-h-0 flex-1">
					{error ? <SinCamara error={error} /> : <Visor viendo={viendo} pantalla={pantalla} />}
					{estado && (
						<p className="absolute inset-x-0 bottom-3 mx-auto w-fit max-w-[90%] rounded-full bg-black/60 px-3 py-1.5 text-center text-xs text-white">
							{estado}
						</p>
					)}
					{aviso && (
						<p className="absolute inset-x-0 top-3 mx-auto w-fit max-w-[90%] rounded-lg bg-destructive px-3 py-1.5 text-center text-xs text-white">
							{aviso}
						</p>
					)}
				</div>
				{items.length > 0 && (
					<div className="flex max-h-[40%] shrink-0 flex-col border-t bg-background/95 landscape:max-h-none landscape:w-80 landscape:border-t-0 landscape:border-l">
						<ScrollArea className="min-h-0 flex-1">
							<Bandeja items={[...items].reverse()} compacta />
						</ScrollArea>
						<div className="flex shrink-0 items-center gap-2 border-t p-2">
							<span className="mr-auto truncate text-xs text-muted-foreground">
								{t("{n} cartas reconocidas", { n })}
							</span>
							{modo.tipo === "deck" && (
								<Button size="sm" onClick={() => void anadir()} disabled={n === 0 || anadiendo}>
									{anadiendo ? (
										<Icono icon="lucide:loader-2" className="animate-spin" />
									) : (
										<Icono icon="lucide:plus" />
									)}
									{t("Añadir al deck")}
								</Button>
							)}
						</div>
					</div>
				)}
			</div>
		</div>
	);
}

/** El marco guía y el contorno de la carta, sobre la cámara. */
function Visor({
	viendo,
	pantalla,
}: {
	viendo: Viendo | null;
	pantalla: { w: number; h: number };
}) {
	const [caja, setCaja] = useState<{ w: number; h: number } | null>(null);
	const ref = useRef<HTMLDivElement>(null);
	// Se vuelve a medir al girar la pantalla (cambia `pantalla`).
	useEffect(() => {
		const el = ref.current;
		if (el) setCaja({ w: el.clientWidth, h: el.clientHeight });
	}, [pantalla]);
	const guia = caja ? marcoGuia(caja.w, caja.h) : null;
	const contorno =
		caja && viendo?.esquinas
			? encajar(
					viendo.esquinas,
					viendo.ancho,
					viendo.alto,
					pantalla,
					ref.current?.getBoundingClientRect(),
				)
			: null;
	return (
		<div ref={ref} className="absolute inset-0">
			{caja && (
				<svg className="absolute inset-0 size-full" viewBox={`0 0 ${caja.w} ${caja.h}`} aria-hidden>
					{guia && (
						<rect
							x={guia.x}
							y={guia.y}
							width={guia.w}
							height={guia.h}
							rx={guia.w * 0.05}
							fill="none"
							stroke="white"
							strokeOpacity={0.7}
							strokeWidth={2}
							strokeDasharray="10 8"
						/>
					)}
					{contorno && (
						<polygon
							points={contorno.map((p: Punto) => `${p.x},${p.y}`).join(" ")}
							fill="none"
							stroke={viendo?.leyendo ? "#f59e0b" : "var(--primary)"}
							strokeWidth={3}
							strokeLinejoin="round"
						/>
					)}
				</svg>
			)}
		</div>
	);
}

/** Sin cámara o sin permiso: lo dice y deja ir a los ajustes o a la galería. */
function SinCamara({ error }: { error: unknown }) {
	const { t, mensaje } = useLang();
	const sinPermiso =
		(error as { params?: { motivo?: string } })?.params?.motivo === "permiso_camara";
	return (
		<div className="flex h-full flex-col items-center justify-center gap-2.5 bg-background p-4 text-center">
			<Icono icon="lucide:camera-off" className="size-8 text-muted-foreground" />
			<p className="max-w-80 text-xs">{mensaje(error)}</p>
			<div className="flex flex-wrap justify-center gap-1.5">
				{sinPermiso && (
					<Button size="sm" onClick={() => void abrirAjustesDeLaCamara()}>
						<Icono icon="lucide:settings" />
						{t("Abrir los ajustes")}
					</Button>
				)}
				<Button size="sm" variant="outline" onClick={() => void elegirFotos(t("Imágenes"))}>
					<Icono icon="lucide:images" />
					{t("Elegir de la galería")}
				</Button>
			</div>
		</div>
	);
}
