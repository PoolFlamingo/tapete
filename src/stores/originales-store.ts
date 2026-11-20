import { create } from "zustand";
import type { IdJugador } from "@/bindings/IdJugador";
import type { OriginalMesa } from "@/bindings/OriginalMesa";
import type { RefImagen } from "@/bindings/RefImagen";
import type { VistaCarta } from "@/bindings/VistaCarta";

/**
 * **Originales en mi idioma** (E9) en pantalla: de qué jugadores se ven las
 * cartas con la impresión de Scryfall en el idioma de la interfaz en vez de
 * sus proxies, y las caras que Rust ya resolvió para cada carta. Es solo de
 * esta pantalla (no se manda a nadie) y se olvida al cerrar la mesa.
 */
type OriginalesStore = {
	/** Los jugadores cuyas cartas se ven así. */
	modos: Record<number, true>;
	/** Las impresiones de Scryfall de las cartas sin su imagen propia (una repetición
	 *  que la guardó sin ella): se ven con su original, sea de quien sea. */
	sinImagen: Record<string, true>;
	/** Las caras en mi idioma de cada impresión de Scryfall (vacío: no hay otra). */
	caras: Record<string, RefImagen[]>;
	/** El idioma (de cartas) de `caras`; si cambia la interfaz, se rehace. */
	idioma: string | null;
	activar: (jugador: IdJugador, si: boolean) => void;
	recibir: (o: OriginalMesa) => void;
	ponerSinImagen: (ids: string[]) => void;
	/** Tira lo resuelto (otro idioma). */
	olvidarCaras: (idioma: string) => void;
	limpiar: () => void;
};

export const useOriginales = create<OriginalesStore>((set, get) => ({
	modos: {},
	sinImagen: {},
	caras: {},
	idioma: null,
	activar: (jugador, si) => {
		const modos = { ...get().modos };
		if (si) modos[jugador] = true;
		else delete modos[jugador];
		set({ modos });
	},
	recibir: (o) => set({ caras: { ...get().caras, [o.scryfallId]: o.caras } }),
	ponerSinImagen: (ids) => set({ sinImagen: Object.fromEntries(ids.map((id) => [id, true])) }),
	olvidarCaras: (idioma) => set({ caras: {}, idioma }),
	limpiar: () => set({ modos: {}, sinImagen: {}, caras: {}, idioma: null }),
}));

/** Si esta carta se ve con su original: por el modo de su dueño o porque no tiene su imagen. */
export function quiereOriginal(
	c: VistaCarta,
	o: Pick<OriginalesStore, "modos" | "sinImagen">,
): boolean {
	const id = c.origen?.scryfallId;
	return id != null && (o.modos[c.dueno] === true || o.sinImagen[id] === true);
}

/**
 * La carta tal como se ve con el modo de su dueño (o sin su imagen propia):
 * si es de Scryfall y ya se resolvió su impresión en mi idioma, esas caras
 * (las que falten, las que ya tenía). Si no hay nada que cambiar, la misma carta.
 */
export function conOriginales(
	c: VistaCarta,
	o: Pick<OriginalesStore, "modos" | "sinImagen" | "caras">,
): VistaCarta {
	const id = c.origen?.scryfallId;
	if (!c.origen || !id || !quiereOriginal(c, o)) return c;
	const nuevas = o.caras[id];
	if (!nuevas || nuevas.length === 0) return c;
	const caras = c.origen.caras.map((base, i) => nuevas[i] ?? base);
	if (caras.every((x, i) => x === c.origen?.caras[i])) return c;
	return { ...c, origen: { ...c.origen, caras } };
}
