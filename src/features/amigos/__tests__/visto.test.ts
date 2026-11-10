import { describe, expect, it } from "vitest";
import { vistoHace } from "../visto";

const AHORA = Date.parse("2026-11-10T12:00:00Z");

describe("cuándo se vio a un amigo", () => {
	it("lo dice en el idioma, redondeando a la unidad que toca", () => {
		expect(vistoHace("2026-11-10T11:59:30Z", "es-ES", AHORA)).toBe("ahora");
		expect(vistoHace("2026-11-10T11:55:00Z", "es-ES", AHORA)).toBe("hace 5 minutos");
		expect(vistoHace("2026-11-10T09:00:00Z", "en-US", AHORA)).toBe("3 hours ago");
		expect(vistoHace("2026-11-09T12:00:00Z", "es-ES", AHORA)).toBe("ayer");
	});

	it("sin fecha, o con una que no se entiende, nada", () => {
		expect(vistoHace(null, "es-ES", AHORA)).toBeNull();
		expect(vistoHace("cuando sea", "es-ES", AHORA)).toBeNull();
	});

	it("una fecha del futuro (relojes desfasados) es «ahora»", () => {
		expect(vistoHace("2026-11-10T12:05:00Z", "es-ES", AHORA)).toBe("ahora");
	});
});
