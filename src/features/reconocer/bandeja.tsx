/**
 * La bandeja de «Identificar por foto»: una fila por foto (o por carta que
 * reconoce el escáner) con la foto enderezada, la impresión propuesta al lado
 * para compararlas, cómo se supo, un selector para corregirla y, en el modo
 * deck, las copias. La comparten el diálogo y el escáner en vivo.
 */
import type { CardSummary } from "@/bindings/CardSummary";
import { CampoNumero, type Opcion, Selector } from "@/components/campos";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { WithTooltip } from "@/components/ui/with-tooltip";
import { urlBusqueda } from "@/lib/image-url";
import { cn } from "@/lib/utils";
import { useBusqueda } from "@/stores/busqueda-store";
import { useReconocer } from "@/stores/reconocer-store";
import { cartaElegida, type ItemReconocido, opciones } from "./cuentas";

/** Cuántas impresiones se ofrecen en el selector (las más parecidas primero). */
const MAX_OPCIONES = 40;

function etiqueta(c: CardSummary): string {
	const idioma = c.lang && c.lang !== "en" ? ` · ${c.lang.toUpperCase()}` : "";
	return `${c.printedName ?? c.name} · ${c.set.toUpperCase()} ${c.collectorNumber}${idioma}`;
}

export function Bandeja({
	items,
	compacta = false,
}: {
	items: ItemReconocido[];
	compacta?: boolean;
}) {
	return (
		<ul className="flex flex-col gap-1.5 p-1">
			{items.map((i) => (
				<Fila key={i.clave} item={i} compacta={compacta} />
			))}
		</ul>
	);
}

function Fila({ item, compacta }: { item: ItemReconocido; compacta: boolean }) {
	const { t, mensaje } = useLang();
	const modo = useReconocer((s) => s.modo);
	const r = item.reconocida;
	const elegida = cartaElegida(item);
	const quitar = () => useReconocer.getState().quitar(item.clave);

	const como = (() => {
		if (!r) return "";
		if (!elegida) return t("No se ha reconocido: elige la carta.");
		if (r.confianza === "probable") return t("Revísala: el nombre no se leyó del todo.");
		switch (r.por) {
			case "pie":
				return t("Exacta: por la colección y el número del pie.");
			case "imagen":
				return t("Por la ilustración: comprueba la impresión.");
			default:
				return t("Solo por el nombre: elige la impresión.");
		}
	})();
	const aviso = !elegida || r?.confianza === "probable";

	const todas = opciones(item);
	const nImpresiones = r?.impresiones.length ?? 0;
	const ops: Opcion<string>[] = todas
		.filter((_, i) => i < MAX_OPCIONES || todas[i].id === item.elegida)
		.map((c) => ({
			valor: c.id,
			etiqueta: etiqueta(c),
			grupo: todas.indexOf(c) < nImpresiones ? t("Impresiones") : t("Otras cartas"),
		}));

	return (
		<li className="flex items-start gap-2.5 rounded-lg border bg-card p-2">
			<div className="flex shrink-0 gap-1">
				{r?.miniatura ? (
					<img src={r.miniatura} alt={t("Foto")} className="w-12 rounded-sm object-contain" />
				) : (
					<div className="flex aspect-[63/88] w-12 items-center justify-center rounded-sm bg-muted">
						{item.estado === "leyendo" ? (
							<Icono icon="lucide:loader-2" className="size-4 animate-spin" />
						) : (
							<Icono icon="lucide:image-off" className="size-4 text-muted-foreground" />
						)}
					</div>
				)}
				{elegida?.thumb && !compacta && (
					<img
						src={urlBusqueda(elegida.thumb)}
						alt={elegida.name}
						className="w-12 rounded-sm object-contain"
					/>
				)}
			</div>
			<div className="flex min-w-0 flex-1 flex-col gap-1">
				{item.estado === "leyendo" ? (
					<p className="truncate text-xs text-muted-foreground">
						{item.nombre ? `${item.nombre} · ` : ""}
						{t("Leyendo la carta…")}
					</p>
				) : item.estado === "fallo" ? (
					<p className="text-xs text-destructive">
						{item.nombre ? `${item.nombre}: ` : ""}
						{mensaje(item.error)}
					</p>
				) : (
					<>
						<p className="truncate text-xs font-medium">
							{elegida ? (elegida.printedName ?? elegida.name) : t("Sin reconocer")}
						</p>
						<p
							className={cn(
								"text-[11px]",
								aviso ? "text-amber-600 dark:text-amber-400" : "text-muted-foreground",
							)}
						>
							{como}
						</p>
						{ops.length > 0 && (
							<Selector
								valor={item.elegida ?? ""}
								opciones={ops}
								onChange={(id) => useReconocer.getState().elegir(item.clave, id)}
								ancho="w-full max-w-80"
								ariaLabel={t("Carta")}
							/>
						)}
					</>
				)}
			</div>
			<div className="flex shrink-0 flex-col items-end gap-1">
				<WithTooltip content={t("Quitar")}>
					<Button size="icon-sm" variant="ghost" aria-label={t("Quitar")} onClick={quitar}>
						<Icono icon="lucide:x" />
					</Button>
				</WithTooltip>
				{item.estado === "listo" && elegida && modo.tipo === "deck" && (
					<CampoNumero
						valor={item.cantidad}
						onChange={(v) => useReconocer.getState().cantidad(item.clave, v ?? 1)}
						min={1}
						max={99}
						paso={1}
						ancho="w-14"
						ariaLabel={t("Copias")}
					/>
				)}
				{item.estado === "listo" && elegida && modo.tipo === "buscar" && (
					<WithTooltip content={t("Ver la ficha")}>
						<Button
							size="icon-sm"
							variant="outline"
							aria-label={t("Ver la ficha")}
							onClick={() => useBusqueda.getState().abrirFicha(elegida.id)}
						>
							<Icono icon="lucide:info" />
						</Button>
					</WithTooltip>
				)}
			</div>
		</li>
	);
}
