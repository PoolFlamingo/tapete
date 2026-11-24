import { type RefObject, useEffect, useRef } from "react";

/**
 * **El efecto 3D de la mesa** (Ajustes > Mesa > «Efecto 3D», apagado de
 * fábrica): la carta que se arrastra se levanta del tapete y se inclina con
 * el movimiento, como una carta que se sostiene, y baja al soltarla. Es solo
 * de pantalla: Rust y el motor no saben nada de él.
 *
 * La inclinación se escribe en variables CSS del contenedor (`--rx`, `--ry`,
 * `--alto`) en cada fotograma, sin pasar por React: el arrastre ya repinta lo
 * suyo con cada movimiento del puntero y esto no debe añadir más.
 */

/** Cuánto se nota el efecto (Ajustes > Mesa > «Profundidad del efecto 3D»). */
export type Intensidad3d = "suave" | "media" | "intensa" | "extrema";

/** Lo que cambia cada nivel de profundidad. */
export type Preset3d = {
	/** Distancia de la perspectiva, en px: cuanto menor, más se deforma al inclinarse. */
	perspectiva: number;
	/** Lo más que se inclina la carta arrastrada, en grados. */
	inclinacion: number;
	/** Grados por cada px/s de velocidad. */
	porVelocidad: number;
	/** Lo que crece al cogerla (0,07 = un 7 %). */
	escala: number;
	/** Multiplica la longitud y el difuminado de las sombras. */
	sombra: number;
	/** Lo que se echa hacia atrás la mano, en grados. */
	mano: number;
};

export const PRESETS_3D: Record<Intensidad3d, Preset3d> = {
	suave: {
		perspectiva: 1300,
		inclinacion: 9,
		porVelocidad: 0.008,
		escala: 0.04,
		sombra: 0.7,
		mano: 8,
	},
	media: {
		perspectiva: 900,
		inclinacion: 16,
		porVelocidad: 0.012,
		escala: 0.07,
		sombra: 1,
		mano: 14,
	},
	intensa: {
		perspectiva: 650,
		inclinacion: 24,
		porVelocidad: 0.017,
		escala: 0.11,
		sombra: 1.4,
		mano: 22,
	},
	extrema: {
		perspectiva: 450,
		inclinacion: 32,
		porVelocidad: 0.022,
		escala: 0.16,
		sombra: 1.9,
		mano: 30,
	},
};

export const INTENSIDADES_3D: Intensidad3d[] = ["suave", "media", "intensa", "extrema"];

/** Lo que se levanta al cogerla (1 = la escala y la sombra enteras del preset). */
export const ALTO_COGIDA = 1;

/**
 * La inclinación que corresponde a una velocidad (px/s): hacia donde se mueve,
 * el borde de delante baja y el de detrás sube (gira sobre el punto por donde
 * se coge). Con el tope del preset, para que no se vuelva de canto.
 */
export function inclinacionDe(
	vx: number,
	vy: number,
	p: Preset3d = PRESETS_3D.media,
): { rx: number; ry: number } {
	const tope = (v: number) => Math.max(-p.inclinacion, Math.min(p.inclinacion, v));
	// `+ 0` convierte el -0 de una velocidad nula en 0.
	return { rx: tope(-vy * p.porVelocidad) + 0, ry: tope(vx * p.porVelocidad) + 0 };
}

/** Un paso del muelle: se acerca una fracción a su objetivo en cada fotograma. */
export function acercar(actual: number, objetivo: number, fraccion = 0.22): number {
	const n = actual + (objetivo - actual) * fraccion;
	return Math.abs(objetivo - n) < 0.01 ? objetivo : n;
}

type Estado = {
	x: number;
	y: number;
	t: number;
	vx: number;
	vy: number;
	rx: number;
	ry: number;
	alto: number;
	cogida: boolean;
	raf: number;
};

/**
 * Anima la inclinación y la altura de lo que se arrastra en `ref`, que sigue al
 * punto `x`, `y`. `cogida`: aún en la mano (al soltarla baja al tapete). Sin
 * `activo` no hace nada (el efecto apagado o «Reducir animaciones»).
 */
export function useInclinacion(
	ref: RefObject<HTMLElement | null>,
	x: number,
	y: number,
	cogida: boolean,
	activo: boolean,
	preset: Preset3d,
): void {
	const e = useRef<Estado>({
		x,
		y,
		t: performance.now(),
		vx: 0,
		vy: 0,
		rx: 0,
		ry: 0,
		alto: 0,
		cogida,
		raf: 0,
	});

	useEffect(() => {
		if (!activo) return;
		const s = e.current;
		const ahora = performance.now();
		// Los eventos del puntero pueden llegar juntos: un tiempo mínimo evita
		// velocidades enormes por dividir entre casi nada.
		const dt = Math.max(12, ahora - s.t) / 1000;
		s.vx = s.vx * 0.5 + ((x - s.x) / dt) * 0.5;
		s.vy = s.vy * 0.5 + ((y - s.y) / dt) * 0.5;
		s.x = x;
		s.y = y;
		s.t = ahora;
		s.cogida = cogida;
		const el0 = ref.current;
		if (el0) {
			el0.style.setProperty("--p3d-persp", `${preset.perspectiva}px`);
			el0.style.setProperty("--p3d-escala", String(preset.escala));
			el0.style.setProperty("--p3d-sombra", String(preset.sombra));
		}
		const paso = () => {
			// Parado, la velocidad se va apagando y la carta se endereza.
			s.vx *= 0.82;
			s.vy *= 0.82;
			const objetivo = s.cogida ? inclinacionDe(s.vx, s.vy, preset) : { rx: 0, ry: 0 };
			s.rx = acercar(s.rx, objetivo.rx);
			s.ry = acercar(s.ry, objetivo.ry);
			s.alto = acercar(s.alto, s.cogida ? ALTO_COGIDA : 0, s.cogida ? 0.25 : 0.35);
			const el = ref.current;
			if (el) {
				el.style.setProperty("--rx", s.rx.toFixed(2));
				el.style.setProperty("--ry", s.ry.toFixed(2));
				el.style.setProperty("--alto", s.alto.toFixed(3));
			}
			const quieta =
				Math.abs(s.vx) < 1 &&
				Math.abs(s.vy) < 1 &&
				s.rx === objetivo.rx &&
				s.ry === objetivo.ry &&
				s.alto === (s.cogida ? ALTO_COGIDA : 0);
			s.raf = quieta ? 0 : requestAnimationFrame(paso);
		};
		if (!s.raf) s.raf = requestAnimationFrame(paso);
	}, [ref, x, y, cogida, activo, preset]);

	useEffect(
		() => () => {
			if (e.current.raf) cancelAnimationFrame(e.current.raf);
		},
		[],
	);
}

/**
 * El estilo (con `rounded-carta`) de una carta que se arrastra con el efecto: perspectiva, la
 * inclinación y la escala del contenedor (`useInclinacion`, con las variables del
 * preset `--p3d-*`) y una sombra que se aleja y se difumina cuanto más alta va,
 * desplazada al lado contrario de la inclinación (la luz viene de arriba).
 */
export const ESTILO_LEVANTADA: React.CSSProperties = {
	transform:
		"perspective(var(--p3d-persp, 900px)) rotateX(calc(var(--rx, 0) * 1deg)) rotateY(calc(var(--ry, 0) * 1deg)) scale(calc(1 + var(--alto, 0) * var(--p3d-escala, 0.07)))",
	boxShadow:
		"calc((var(--ry, 0) * -0.8px + var(--alto, 0) * 10px) * var(--p3d-sombra, 1)) calc((var(--rx, 0) * 0.8px + var(--alto, 0) * 22px) * var(--p3d-sombra, 1)) calc((8px + var(--alto, 0) * 26px) * var(--p3d-sombra, 1)) rgba(0, 0, 0, calc(0.35 + var(--alto, 0) * 0.2)), 0 2px 6px rgba(0, 0, 0, 0.4)",
	transformStyle: "preserve-3d",
};

/**
 * Las variables de la mano con el efecto (`MANO_3D` en `carta-mesa.tsx`): la
 * perspectiva, lo que se echa hacia atrás y la sombra del preset.
 */
export function varsMano(p: Preset3d): React.CSSProperties {
	return {
		"--mano-persp": `${Math.round(p.perspectiva * 0.78)}px`,
		"--mano-giro": `${p.mano}deg`,
		"--mano-sombra": String(p.sombra),
	} as React.CSSProperties;
}
