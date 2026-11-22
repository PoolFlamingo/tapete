import { describe, expect, it } from "vitest";
import type { CardSummary } from "@/bindings/CardSummary";
import type { Reconocida } from "@/bindings/Reconocida";
import {
	cambiarCantidad,
	cartaElegida,
	elegir,
	empezar,
	falla,
	leyendo,
	llega,
	opciones,
	paraAnadir,
	quitar,
	total,
} from "../cuentas";

function carta(id: string, name = "Lightning Bolt"): CardSummary {
	return {
		id,
		name,
		printedName: null,
		lang: "en",
		set: "m10",
		setName: "Magic 2010",
		collectorNumber: "146",
		thumb: null,
		highres: true,
		imageStatus: null,
		isDfc: false,
		typeLine: "Instant",
	};
}

function reconocida(
	mejor: string | null,
	impresiones: string[],
	candidatas: string[] = [],
): Reconocida {
	return {
		esquinas: null,
		miniatura: "",
		lectura: [],
		candidatas: candidatas.map((id) => carta(id, "Otra")),
		mejor,
		confianza: mejor ? "exact" : "none",
		impresiones: impresiones.map((id) => ({ carta: carta(id), distancia: null })),
		por: mejor ? "pie" : "nada",
	};
}

describe("la bandeja de identificar por foto", () => {
	it("una foto pasa de leyendo a lista con la carta propuesta", () => {
		let v = empezar([], "f1", "foto.jpg");
		expect(leyendo(v)).toBe(true);
		v = llega(v, "f1", reconocida("a", ["a", "b"]));
		expect(leyendo(v)).toBe(false);
		expect(v).toHaveLength(1);
		expect(v[0].nombre).toBe("foto.jpg");
		expect(cartaElegida(v[0])?.id).toBe("a");
		expect(total(v)).toBe(1);
	});

	it("la misma carta dos veces suma copias en vez de repetirse", () => {
		let v = empezar([], "f1");
		v = llega(v, "f1", reconocida("a", ["a"]));
		v = empezar(v, "f2");
		v = llega(v, "f2", reconocida("a", ["a"]));
		expect(v).toHaveLength(1);
		expect(v[0].cantidad).toBe(2);
		// Lo que llega del escáner en vivo, sin haber empezado antes.
		v = llega(v, "vivo-1", reconocida("b", ["b"]));
		expect(v.map((i) => i.clave)).toEqual(["f1", "vivo-1"]);
		expect(total(v)).toBe(3);
	});

	it("sin propuesta no cuenta hasta que se elige una carta", () => {
		let v = llega([], "f1", reconocida(null, [], ["x", "y"]));
		expect(total(v)).toBe(0);
		expect(opciones(v[0]).map((c) => c.id)).toEqual(["x", "y"]);
		v = elegir(v, "f1", "y");
		expect(paraAnadir(v).map((x) => x.carta.id)).toEqual(["y"]);
	});

	it("las opciones son las impresiones y luego los otros nombres, sin repetir", () => {
		const v = llega([], "f1", reconocida("a", ["a", "b"], ["b", "c"]));
		expect(opciones(v[0]).map((c) => c.id)).toEqual(["a", "b", "c"]);
	});

	it("la cantidad se queda entre 1 y 99", () => {
		let v = llega([], "f1", reconocida("a", ["a"]));
		v = cambiarCantidad(v, "f1", 0);
		expect(v[0].cantidad).toBe(1);
		v = cambiarCantidad(v, "f1", 250);
		expect(v[0].cantidad).toBe(99);
	});

	it("una foto que falla no cuenta y se puede quitar", () => {
		let v = empezar([], "f1");
		v = falla(v, "f1", { code: "Image" });
		expect(v[0].estado).toBe("fallo");
		expect(total(v)).toBe(0);
		expect(quitar(v, "f1")).toEqual([]);
	});
});
