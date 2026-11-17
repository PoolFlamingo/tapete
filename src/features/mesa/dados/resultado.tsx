import { useEffect, useMemo, useState } from "react";
import { useLang } from "@/components/language-provider";
import { claveEstilo, type Estilo, estiloDe, useAspectoDados, useQuienTira } from "./aspecto";
import { type CambioAzar, MAX_PIEZAS, type PiezaTirada } from "./cola";
import { dadosDeTirada } from "./geometria";
import { PiezaQuieta } from "./plano";
import { lienzoFondo, urlFondo } from "./texturas";

/**
 * **El resultado de una tirada**, con las mismas piezas que se tiran en la
 * mesa (los dados y la moneda en 2D, con el color, el acabado, la imagen y
 * el metal de quien tiró): en el cartel que queda tras la tirada
 * (`capa.tsx`) y en el diálogo «Dados y monedas». El resultado es el del
 * motor; aquí solo se pinta.
 */

/** El estilo de las piezas de `actor` y el fondo de sus caras (`null` mientras se pinta). */
export function useEstiloDe(actor: number | null): { estilo: Estilo; fondo: string | null } {
	const { color, mia } = useQuienTira(actor);
	const vista = useAspectoDados((s) => s.vista);
	useEffect(() => {
		void useAspectoDados.getState().cargar();
	}, []);
	const estilo = useMemo(() => estiloDe(vista, color, mia), [vista, color, mia]);
	const clave = claveEstilo(estilo);
	const [fondo, setFondo] = useState<{ clave: string; url: string } | null>(null);
	useEffect(() => {
		let vivo = true;
		void lienzoFondo(estilo).then((l) => {
			if (vivo) setFondo({ clave, url: urlFondo(l) });
		});
		return () => {
			vivo = false;
		};
	}, [estilo, clave]);
	return { estilo, fondo: fondo?.clave === clave ? fondo.url : null };
}

/** Los dados de una tirada de `caras` (el d100, sus dos d10; un dado sin modelo, el d6 con su número). */
function dadosDe(caras: number, valor: number): PiezaTirada[] {
	const conModelo = [4, 6, 8, 10, 12, 20, 100].includes(caras);
	return dadosDeTirada(caras, valor).map((d) => ({
		forma: "dado" as const,
		...d,
		valor: conModelo ? d.valor : valor,
	}));
}

function Piezas({
	piezas,
	estilo,
	fondo,
}: {
	piezas: PiezaTirada[];
	estilo: Estilo;
	fondo: string;
}) {
	return (
		<div className="flex flex-wrap items-center justify-center gap-2">
			{piezas.map((p, i) => (
				<PiezaQuieta key={i} p={p} estilo={estilo} fondo={fondo} />
			))}
		</div>
	);
}

export function ResultadoAzar({
	cambio,
	actor,
}: {
	cambio: CambioAzar | null;
	/** Quién tiró (sus dados); `null`, la mesa. */
	actor: number | null;
}) {
	const { t } = useLang();
	const { estilo, fondo } = useEstiloDe(actor);
	if (!cambio) return null;
	// Mientras se pinta el fondo de las caras (un instante), el hueco.
	if (!fondo) return <div className="h-16" />;
	if (cambio.tipo === "dados") {
		const suma = cambio.tiradas.reduce((a, x) => a + x.valor, 0);
		const piezas = cambio.tiradas.flatMap((x) => dadosDe(x.caras, x.valor));
		const d100 = cambio.tiradas.some((x) => x.caras === 100);
		return (
			<div className="flex flex-col items-center gap-1.5 rounded-lg bg-muted/50 p-2.5">
				<Piezas piezas={piezas.slice(0, MAX_PIEZAS)} estilo={estilo} fondo={fondo} />
				{/* Las que no caben como pieza, en números. */}
				{piezas.length > MAX_PIEZAS && (
					<span className="text-center text-xs text-muted-foreground">
						{cambio.tiradas.map((x) => x.valor).join(" · ")}
					</span>
				)}
				{(cambio.tiradas.length > 1 || d100) && (
					<span className="text-xs text-muted-foreground">{t("Suma: {n}", { n: suma })}</span>
				)}
			</div>
		);
	}
	if (cambio.tipo === "monedas") {
		// Con «hasta perder», una fila por tanda.
		return (
			<div className="flex flex-col items-center gap-1.5 rounded-lg bg-muted/50 p-2.5">
				{cambio.lanzamientos.map((l, i) => (
					<Piezas
						key={i}
						piezas={l.slice(0, MAX_PIEZAS).map((cara) => ({ forma: "moneda" as const, cara }))}
						estilo={estilo}
						fondo={fondo}
					/>
				))}
			</div>
		);
	}
	return (
		<div className="flex items-center justify-center gap-2.5 rounded-lg bg-muted/50 p-2.5">
			<PiezaQuieta p={{ forma: "planar", cara: cambio.cara }} estilo={estilo} fondo={fondo} />
			<div className="flex flex-col text-sm font-semibold">
				{cambio.cara === "planeswalk"
					? t("Planeswalk")
					: cambio.cara === "caos"
						? t("Caos")
						: t("Nada")}
				<span className="text-xs font-normal text-muted-foreground">
					{t("Esta tirada costaba {n}", { n: cambio.coste })}
				</span>
			</div>
		</div>
	);
}
