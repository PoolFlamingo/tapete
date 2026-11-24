import { describe, expect, it } from "vitest";
import type { ListaDeJuego } from "@/bindings/ListaDeJuego";
import { formatoDeDeck } from "../opciones-fabrica";

const lista = (comandantes: string[]): ListaDeJuego => ({
	id: "l",
	nombre: "Main",
	comandantes,
	cartas: [],
});

describe("formato de fábrica de un deck", () => {
	it("sale de la clase que deduce Rust", () => {
		expect(formatoDeDeck("commander", null)).toBe("commander");
		expect(formatoDeDeck("constructed", null)).toBe("constructed");
		expect(formatoDeDeck("limited", [])).toBe("limited");
	});

	it("una lista con comandante lo hace Commander aunque las cartas no lo digan", () => {
		expect(formatoDeDeck("constructed", [lista([]), lista(["c1"])])).toBe("commander");
		expect(formatoDeDeck("constructed", [lista([])])).toBe("constructed");
	});

	it("lo elegido a mano manda sobre todo, también sobre la lista", () => {
		expect(formatoDeDeck("commander", [lista(["c1"])], "constructed")).toBe("constructed");
		expect(formatoDeDeck("constructed", null, "commander")).toBe("commander");
		expect(formatoDeDeck("commander", null, "limited")).toBe("limited");
	});
});
