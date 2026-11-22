import { describe, expect, it } from "vitest";
import { encajar, marcoGuia } from "../visor";

describe("el visor del escáner", () => {
	it("el marco guía es una carta centrada que cabe en la caja", () => {
		const g = marcoGuia(400, 800);
		expect(g.w / g.h).toBeCloseTo(63 / 88);
		expect(g.w).toBeCloseTo(300);
		expect(g.x).toBeCloseTo(50);
		expect(g.y + g.h / 2).toBeCloseTo(400);
		// De lado manda el alto.
		const l = marcoGuia(800, 400);
		expect(l.h).toBeCloseTo(300);
		expect(l.x + l.w / 2).toBeCloseTo(400);
	});

	it("encaja el fotograma como la vista previa, recortando lo que sobra", () => {
		// Un fotograma de 720 × 1280 en una pantalla de 400 × 800: escala 0,625,
		// de 450 × 800, centrado (sobran 25 px por cada lado).
		const [a, b] = encajar(
			[
				{ x: 0, y: 0 },
				{ x: 1, y: 1 },
			],
			720,
			1280,
			{ w: 400, h: 800 },
		);
		expect(a.x).toBeCloseTo(-25);
		expect(a.y).toBeCloseTo(0);
		expect(b.x).toBeCloseTo(425);
		expect(b.y).toBeCloseTo(800);
	});

	it("resta dónde empieza la caja del visor", () => {
		const [p] = encajar([{ x: 0.5, y: 0.5 }], 100, 100, { w: 200, h: 200 }, { left: 0, top: 48 });
		expect(p).toEqual({ x: 100, y: 52 });
	});
});
