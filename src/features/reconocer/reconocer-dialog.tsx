/**
 * **Identificar por foto** (escritorio y galería): soltar o elegir fotos de
 * cartas reales; Rust encuentra la carta en cada una, la lee y propone la
 * impresión. En Buscar, cada una abre su ficha; desde «Añadir carta», entran
 * al deck con sus copias. En el móvil, «Escanear con la cámara» pasa al
 * escáner en vivo con la misma bandeja.
 */
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { DialogoAdaptable } from "@/components/dialogo-adaptable";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogCuerpo,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { EXTENSIONES_IMAGEN } from "@/features/decklist/acciones";
import { soloImagenes } from "@/features/preview/arrastrar-ficheros";
import { abrirParaImportar } from "@/lib/dialogos";
import { useEsMovil } from "@/lib/plataforma";
import { cn } from "@/lib/utils";
import { useReconocer } from "@/stores/reconocer-store";
import { anadirAlDeck } from "./anadir";
import { Bandeja } from "./bandeja";
import { leyendo, total } from "./cuentas";

export function ReconocerDialog() {
	const abierto = useReconocer((s) => s.abierto);
	return (
		<Dialog open={abierto} onOpenChange={(o) => !o && useReconocer.getState().cerrar()}>
			{abierto && <Contenido />}
		</Dialog>
	);
}

/** El nombre de una ruta de fichero (en una URI de Android no hay uno que valga). */
export function nombreDeRuta(ruta: string): string | undefined {
	if (ruta.startsWith("content://")) return undefined;
	return ruta.split(/[\\/]/).pop() || undefined;
}

/** Elegir fotos con el diálogo del sistema (la galería en el móvil) y leerlas. */
export async function elegirFotos(nombreFiltro: string): Promise<void> {
	const rutas = await abrirParaImportar({
		multiple: true,
		filters: [{ name: nombreFiltro, extensions: EXTENSIONES_IMAGEN }],
	});
	if (!rutas?.length) return;
	await useReconocer.getState().leer(rutas.map((ruta) => ({ ruta, nombre: nombreDeRuta(ruta) })));
}

/** Soltar fotos sobre el diálogo (solo escritorio): las lee y dice si hay algo encima. */
function useSoltarFotos(movil: boolean): boolean {
	const [encima, setEncima] = useState(false);
	useEffect(() => {
		if (movil) return;
		let dejar: (() => void) | null = null;
		let vivo = true;
		getCurrentWebview()
			.onDragDropEvent((ev) => {
				const p = ev.payload;
				if (p.type === "enter" || p.type === "over") setEncima(true);
				else if (p.type === "leave") setEncima(false);
				else if (p.type === "drop") {
					setEncima(false);
					const fotos = soloImagenes(p.paths).map((ruta) => ({ ruta, nombre: nombreDeRuta(ruta) }));
					if (fotos.length > 0) void useReconocer.getState().leer(fotos);
				}
			})
			.then((f) => {
				if (vivo) dejar = f;
				else f();
			})
			.catch(() => {
				/* fuera de Tauri (tests) */
			});
		return () => {
			vivo = false;
			dejar?.();
		};
	}, [movil]);
	return encima;
}

function Contenido() {
	const { t, mensaje } = useLang();
	const movil = useEsMovil();
	const items = useReconocer((s) => s.items);
	const modo = useReconocer((s) => s.modo);
	const [anadiendo, setAnadiendo] = useState(false);
	const encima = useSoltarFotos(movil);
	const n = total(items);
	const enCurso = leyendo(items);

	const elegir = () => elegirFotos(t("Imágenes")).catch((e) => toast.error(mensaje(e)));

	const anadir = async () => {
		setAnadiendo(true);
		try {
			if ((await anadirAlDeck(items, modo, t)) > 0) useReconocer.getState().cerrar();
		} catch (e) {
			toast.error(mensaje(e));
		} finally {
			setAnadiendo(false);
		}
	};

	return (
		<DialogoAdaptable
			className="flex max-h-[85vh] flex-col sm:max-w-2xl"
			ocupado={enCurso || anadiendo}
			cambiosSinGuardar={modo.tipo === "deck" && n > 0}
		>
			<DialogHeader>
				<DialogTitle>{t("Identificar por foto")}</DialogTitle>
				<DialogDescription>
					{modo.tipo === "deck"
						? t(
								"Haz o elige fotos de cartas reales: se reconoce cada una y su impresión para añadirlas al deck.",
							)
						: t("Haz o elige fotos de cartas reales: se reconoce cada una y su impresión.")}
				</DialogDescription>
			</DialogHeader>
			<div
				className={cn(
					"flex shrink-0 flex-col items-center gap-2 rounded-lg border-2 border-dashed p-4 text-center transition-colors",
					encima ? "border-primary bg-primary/5" : "border-border",
				)}
			>
				<Icono icon="lucide:scan-line" className="size-6 text-muted-foreground" />
				{!movil && (
					<p className="text-xs text-muted-foreground">
						{t("Suelta aquí fotos de cartas (una carta por foto).")}
					</p>
				)}
				<div className="flex flex-wrap justify-center gap-1.5">
					{movil && (
						<Button size="sm" onClick={() => useReconocer.getState().verVivo(true)}>
							<Icono icon="lucide:camera" />
							{t("Escanear con la cámara")}
						</Button>
					)}
					<Button size="sm" variant={movil ? "outline" : "default"} onClick={() => void elegir()}>
						<Icono icon="lucide:images" />
						{movil ? t("Elegir de la galería") : t("Elegir imágenes…")}
					</Button>
				</div>
			</div>
			<DialogCuerpo className="min-h-32">
				{items.length === 0 ? (
					<p className="py-10 text-center text-xs text-muted-foreground">
						{t("Aún no hay ninguna foto.")}
					</p>
				) : (
					<Bandeja items={items} />
				)}
			</DialogCuerpo>
			<DialogFooter className="items-center">
				<span className="mr-auto truncate text-xs text-muted-foreground">
					{enCurso ? t("Leyendo…") : items.length > 0 ? t("{n} cartas reconocidas", { n }) : ""}
				</span>
				{items.length > 0 && (
					<Button size="sm" variant="outline" onClick={() => useReconocer.getState().vaciar()}>
						{t("Vaciar")}
					</Button>
				)}
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
			</DialogFooter>
		</DialogoAdaptable>
	);
}
