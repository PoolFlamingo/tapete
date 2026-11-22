/** Meter en el deck lo reconocido (desde el diálogo o desde el escáner en vivo). */
import type { DeckCard } from "@/bindings/DeckCard";
import type { T } from "@/features/decklist/acciones";
import { traerCarta } from "@/features/variants/traer";
import { insertar, nuevoId, quitar } from "@/lib/deck-ops";
import { useDeck } from "@/stores/deck-store";
import type { ModoReconocer } from "@/stores/reconocer-store";
import { useUi } from "@/stores/ui-store";
import { type ItemReconocido, paraAnadir } from "./cuentas";

/**
 * Añade cada carta elegida con sus copias, como «Añadir carta»: en la posición
 * de la que se abrió y, si se abrió desde una casilla en blanco, en su lugar.
 * Devuelve cuántas copias entraron.
 */
export async function anadirAlDeck(
	items: ItemReconocido[],
	modo: ModoReconocer,
	t: T,
): Promise<number> {
	const nuevas: DeckCard[] = [];
	for (const { carta, cantidad } of paraAnadir(items)) {
		const base = await traerCarta(carta);
		for (let i = 0; i < cantidad; i++) nuevas.push(i === 0 ? base : { ...base, id: nuevoId() });
	}
	if (nuevas.length === 0) return 0;
	const en = modo.tipo === "deck" ? modo.en : undefined;
	const hueco = modo.tipo === "deck" ? modo.hueco : undefined;
	useDeck.getState().setCartas((cs) => insertar(hueco ? quitar(cs, [hueco]) : cs, nuevas, en));
	useUi.getState().anunciar(t("{n} cartas añadidas al deck", { n: nuevas.length }));
	return nuevas.length;
}
