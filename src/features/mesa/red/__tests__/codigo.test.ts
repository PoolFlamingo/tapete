import { describe, expect, it } from "vitest";
import { limpiarCodigoDirecto } from "../codigo";

describe("el código de partida directa pegado", () => {
	it("quita el principio del enlace y los espacios, y pone el prefijo en mayúsculas", () => {
		expect(limpiarCodigoDirecto(" proxybuilder://unirse/pb1-AbC dEf\n")).toBe("PB1-AbCdEf");
		expect(limpiarCodigoDirecto("tapete://unirse/pb1-AbCdEf")).toBe("PB1-AbCdEf");
		expect(limpiarCodigoDirecto("pb1-xYz")).toBe("PB1-xYz");
		expect(limpiarCodigoDirecto("PB1-xYz")).toBe("PB1-xYz");
	});

	it("acepta el enlace de la web, con o sin lo que vaya detrás", () => {
		expect(limpiarCodigoDirecto("https://tapete.zodhub.app/join/PB1-AbC_d-9")).toBe("PB1-AbC_d-9");
		expect(limpiarCodigoDirecto("https://Tapete.zodhub.app/join/pb1-AbC/?utm=x#y")).toBe("PB1-AbC");
	});
});
