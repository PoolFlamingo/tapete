import type { DeckKind } from "@/bindings/DeckKind";
import type { Formato } from "@/bindings/Formato";
import type { ListaDeJuego } from "@/bindings/ListaDeJuego";
import type { ModoEquipos } from "@/bindings/ModoEquipos";
import type { Opciones } from "@/bindings/Opciones";

/** Los valores de fábrica de un formato (lo mismo que `Opciones::de_fabrica` de Rust). */
export function opcionesDeFabrica(
	formato: Formato,
	jugadores: number,
	equipos?: ModoEquipos,
): Opciones {
	return {
		vidaInicial: equipos?.tipo === "dosCabezas" ? 30 : formato === "commander" ? 40 : 20,
		manoInicial: 7,
		mulligan: formato === "commander" ? "londresPrimeroGratis" : "londres",
		primeroNoRoba: jugadores <= 2,
		ayudas: {
			robarAlEmpezar: true,
			enderezarAlEmpezar: true,
			impuestoAutomatico: true,
			contadoresAlEntrar: true,
			avisarVidaCero: true,
		},
		limiteVeneno: 10,
		limiteComandante: 21,
		todosTocanTodo: false,
		campoAjenoProtegido: false,
	};
}

/**
 * El formato de fábrica para un deck: el elegido a mano (`manual`, que ya
 * trae también `DeckDetails.kind`) manda; si no, Commander si tiene una lista
 * para jugar con comandante, que lo dice sin dudas aunque el deck pase de largo
 * de 100 con alternativas; si no, el que deduce Rust de sus cartas.
 */
export function formatoDeDeck(
	kind: DeckKind,
	listas: ListaDeJuego[] | null,
	manual: DeckKind | null = null,
): Formato {
	if (manual) kind = manual;
	else if (listas?.some((l) => l.comandantes.length > 0)) return "commander";
	return kind === "commander" ? "commander" : kind === "limited" ? "limited" : "constructed";
}
