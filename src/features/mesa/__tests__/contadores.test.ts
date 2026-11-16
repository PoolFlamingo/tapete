import { describe, expect, it } from "vitest";
import { alternarFavorito, MAX_PROPIOS, recordarPropio, tipoEscrito } from "../contadores";

const t = (s: string) => (s === "Escudo" ? "Escudo" : s);

describe("contadores propios y favoritos", () => {
	it("solo recuerda los que no son del catálogo", () => {
		expect(recordarPropio([], "+1/+1")).toEqual([]);
		expect(recordarPropio([], "shield")).toEqual([]);
		expect(recordarPropio([], "+2/+2")).toEqual([]);
		expect(recordarPropio([], "Ánimo")).toEqual(["Ánimo"]);
	});

	it("los recuerda sin el prefijo de temporal y el último va primero, sin repetirse", () => {
		const a = recordarPropio([], "~Ánimo");
		expect(a).toEqual(["Ánimo"]);
		const b = recordarPropio(a, "Calma");
		expect(b).toEqual(["Calma", "Ánimo"]);
		expect(recordarPropio(b, "ánimo")).toEqual(["ánimo", "Calma"]);
	});

	it("no pasa del tope", () => {
		let l: string[] = [];
		for (let i = 0; i < MAX_PROPIOS + 5; i++) l = recordarPropio(l, `c${i}`);
		expect(l).toHaveLength(MAX_PROPIOS);
		expect(l[0]).toBe(`c${MAX_PROPIOS + 4}`);
	});

	it("lo escrito que coincide con el catálogo es ese tipo", () => {
		expect(tipoEscrito("  escudo ", t)).toBe("shield");
		expect(tipoEscrito("SHIELD", t)).toBe("shield");
		expect(tipoEscrito("Ánimo", t)).toBe("Ánimo");
		expect(tipoEscrito("   ", t)).toBe("");
		// Las palabras clave no son contadores: «flying» escrito es uno propio.
		expect(tipoEscrito("flying", t)).toBe("flying");
	});

	it("alterna favoritos sin el prefijo de temporal", () => {
		const a = alternarFavorito([], "~+1/+1");
		expect(a).toEqual(["+1/+1"]);
		expect(alternarFavorito(a, "+1/+1")).toEqual([]);
	});
});
