import { describe, expect, it } from "vitest";
import { idiomaDelSelector } from "../idioma";
import { insertarEnCursor } from "../insertar";

describe("meter un emoji donde está el cursor", () => {
	it("en medio, con el cursor suelto", () => {
		expect(insertarEnCursor("holamundo", 4, 4, "👋")).toEqual({ texto: "hola👋mundo", cursor: 6 });
	});
	it("sustituye lo seleccionado", () => {
		expect(insertarEnCursor("hola mundo", 5, 10, "🌍")).toEqual({ texto: "hola 🌍", cursor: 7 });
	});
	it("sin cursor conocido, al final", () => {
		expect(insertarEnCursor("hola", null, null, "!")).toEqual({ texto: "hola!", cursor: 5 });
	});
	it("posiciones fuera de rango se recortan", () => {
		expect(insertarEnCursor("ab", 9, 1, "x")).toEqual({ texto: "abx", cursor: 3 });
		expect(insertarEnCursor("ab", -3, -1, "x")).toEqual({ texto: "xab", cursor: 1 });
	});
	it("si no cabe, no cambia nada (ni medio emoji)", () => {
		expect(insertarEnCursor("abc", 3, 3, "👍", 4)).toBeNull();
		expect(insertarEnCursor("abc", 3, 3, "👍", 5)).toEqual({ texto: "abc👍", cursor: 5 });
		// Sustituir una selección deja sitio.
		expect(insertarEnCursor("abcd", 0, 2, "👍", 4)).toEqual({ texto: "👍cd", cursor: 2 });
	});
});

describe("el idioma del selector", () => {
	it("los que trae emoji-mart van tal cual", () => {
		expect(idiomaDelSelector("es")).toBe("es");
		expect(idiomaDelSelector("uk")).toBe("uk");
		expect(idiomaDelSelector("ja")).toBe("ja");
	});
	it("los dos chinos usan el suyo y el rumano, que no está, va en inglés", () => {
		expect(idiomaDelSelector("zhs")).toBe("zh");
		expect(idiomaDelSelector("zht")).toBe("zh");
		expect(idiomaDelSelector("ro")).toBe("en");
	});
});
