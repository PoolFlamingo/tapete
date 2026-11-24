import { describe, expect, it } from "vitest";
import { acercar, INTENSIDADES_3D, inclinacionDe, PRESETS_3D } from "../profundidad";

describe("efecto 3D de la carta arrastrada", () => {
	it("quieta no se inclina", () => {
		expect(inclinacionDe(0, 0)).toEqual({ rx: 0, ry: 0 });
	});

	it("se inclina hacia donde se mueve, con el tope del preset", () => {
		const derecha = inclinacionDe(500, 0);
		expect(derecha.ry).toBeGreaterThan(0);
		expect(derecha.rx).toBe(0);
		expect(inclinacionDe(0, -500).rx).toBeGreaterThan(0);
		for (const i of INTENSIDADES_3D) {
			const p = PRESETS_3D[i];
			expect(inclinacionDe(1e6, -1e6, p)).toEqual({ rx: p.inclinacion, ry: p.inclinacion });
			expect(inclinacionDe(-1e6, 1e6, p)).toEqual({ rx: -p.inclinacion, ry: -p.inclinacion });
		}
	});

	it("cada nivel se nota más que el anterior", () => {
		for (let k = 1; k < INTENSIDADES_3D.length; k++) {
			const antes = PRESETS_3D[INTENSIDADES_3D[k - 1] ?? "media"];
			const ahora = PRESETS_3D[INTENSIDADES_3D[k] ?? "media"];
			expect(ahora.perspectiva).toBeLessThan(antes.perspectiva);
			expect(ahora.inclinacion).toBeGreaterThan(antes.inclinacion);
			expect(ahora.escala).toBeGreaterThan(antes.escala);
			expect(ahora.sombra).toBeGreaterThan(antes.sombra);
			expect(ahora.mano).toBeGreaterThan(antes.mano);
			expect(inclinacionDe(300, 0, ahora).ry).toBeGreaterThan(inclinacionDe(300, 0, antes).ry);
		}
	});

	it("el muelle llega a su objetivo y se queda", () => {
		let v = 0;
		for (let i = 0; i < 100; i++) v = acercar(v, 10);
		expect(v).toBe(10);
		expect(acercar(10, 10)).toBe(10);
	});
});
