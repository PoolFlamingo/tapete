import { useEffect, useId, useMemo, useRef } from "react";
import { type Estilo, METALES } from "./aspecto";
import { entre, generador, giroAlAzar } from "./azar";
import type { PiezaTirada, Tirada } from "./cola";
import { esquinasD4, geometria, giroDeCara, type M3, planoCara, textoCara } from "./geometria";
import { caraDelResultado, tipoDe } from "./piezas";
import { CAOS, CARA, CRUZ, PLANESWALK } from "./simbolos";
import { golpeDado, golpeMoneda, lanzarMoneda } from "./sonidos";
import { LADO_FONDO, trozoDeCara } from "./texturas";

/**
 * **Los dados «3D ligero»**, low poly, sin WebGL ni física, y **sin el 3D
 * del navegador**: cada fotograma gira el sólido aquí (`giro`), se quedan
 * las caras que miran a quien juega (en un sólido convexo nunca se tapan
 * entre sí, así que no hay que ordenarlas) y cada una se pinta en SVG con la
 * transformación afín que la lleva a la pantalla (proyección ortográfica).
 *
 * Antes eran elementos con `matrix3d` y `transform-style: preserve-3d`, pero
 * WebKitGTK sin aceleración (`HardwareAccelerationPolicy` NEVER, como en
 * equipos sin GPU que le guste) no compone en 3D: aplanaba cada cara por
 * separado, el d6 desaparecía y los demás salían aplastados. Esto se ve
 * igual en cualquier vista web.
 *
 * La tirada cae, bota y se queda con la cara del resultado mirando a quien
 * juega, algo inclinada (`INCLINACION`) para que se vea el volumen. La luz
 * se calcula en cada fotograma con la normal girada. El aspecto (`Estilo`)
 * pone el fondo de las caras (`fondo`, del mismo lienzo que el 3D) y el color
 * de los números; el translúcido aquí es solo un poco de transparencia.
 */

/** Píxeles por unidad de la geometría (los dados miden ~1,2). */
const ESCALA = 66;
const DURA = 1300;
/** Los momentos clave de la animación y la altura en cada uno: cae, bota y se queda. */
const MOMENTOS = [0, 0.4, 0.62, 0.8, 1];
const ALTO = [80, 0, 16, 0, 0];
/** Lo que se ha recorrido del camino en cada momento. */
const CAMINO = [0, 0.6, 0.82, 0.96, 1];
/** Los golpes: en qué momento y con qué fuerza. */
const GOLPES: [number, number][] = [
	[1, 0.85],
	[3, 0.4],
];
const EASING: [number, number, number, number] = [0.2, 0.7, 0.3, 1];
/** Lo que se inclina la pose final hacia quien mira (en radianes, sobre X). */
const INCLINACION = -0.42;
/** La moneda: diámetro y grosor en píxeles. */
const MONEDA = 76;
const GROSOR = 10;

/** El punto de la curva `EASING` (de Bézier) en `s`, para `a`/`b` de un eje. */
const curva = (a: number, b: number, s: number) =>
	3 * a * s * (1 - s) ** 2 + 3 * b * s * s * (1 - s) + s ** 3;

/** El `s` en que la curva vale `v` en el eje (`a`, `b`), por bisección. */
function inversa(a: number, b: number, v: number): number {
	let lo = 0;
	let hi = 1;
	for (let i = 0; i < 30; i++) {
		const m = (lo + hi) / 2;
		if (curva(a, b, m) < v) lo = m;
		else hi = m;
	}
	return (lo + hi) / 2;
}

/** Del tiempo (0–1) al avance de la animación (0–1), con el `EASING`. */
function avance(t: number): number {
	const [x1, y1, x2, y2] = EASING;
	return curva(y1, y2, inversa(x1, x2, t));
}

/** El tiempo (0–1) en que la animación llega al avance `p`: la inversa de `avance`. */
function tiempoDe(p: number): number {
	const [x1, y1, x2, y2] = EASING;
	return curva(x1, x2, inversa(y1, y2, p));
}

/** En qué tramo de `MOMENTOS` cae el avance `p` y cuánto lleva de él. */
function tramo(p: number): [number, number] {
	for (let k = 0; k < MOMENTOS.length - 1; k++) {
		if (p <= MOMENTOS[k + 1]) return [k, (p - MOMENTOS[k]) / (MOMENTOS[k + 1] - MOMENTOS[k])];
	}
	return [MOMENTOS.length - 2, 1];
}

const mezcla = (v: number[], k: number, f: number) => v[k] + (v[k + 1] - v[k]) * f;

type Q = [number, number, number, number];

const deCuaternion = ([x, y, z, w]: Q): M3 => [
	1 - 2 * (y * y + z * z),
	2 * (x * y - z * w),
	2 * (x * z + y * w),
	2 * (x * y + z * w),
	1 - 2 * (x * x + z * z),
	2 * (y * z - x * w),
	2 * (x * z - y * w),
	2 * (y * z + x * w),
	1 - 2 * (x * x + y * y),
];

/** El cuaternión de un giro (la inversa de `deCuaternion`). */
function aCuaternion(m: M3): Q {
	const tr = m[0] + m[4] + m[8];
	let q: Q;
	if (tr > 0) {
		const s = Math.sqrt(tr + 1) * 2;
		q = [(m[7] - m[5]) / s, (m[2] - m[6]) / s, (m[3] - m[1]) / s, s / 4];
	} else if (m[0] > m[4] && m[0] > m[8]) {
		const s = Math.sqrt(1 + m[0] - m[4] - m[8]) * 2;
		q = [s / 4, (m[1] + m[3]) / s, (m[2] + m[6]) / s, (m[7] - m[5]) / s];
	} else if (m[4] > m[8]) {
		const s = Math.sqrt(1 + m[4] - m[0] - m[8]) * 2;
		q = [(m[1] + m[3]) / s, s / 4, (m[5] + m[7]) / s, (m[2] - m[6]) / s];
	} else {
		const s = Math.sqrt(1 + m[8] - m[0] - m[4]) * 2;
		q = [(m[2] + m[6]) / s, (m[5] + m[7]) / s, s / 4, (m[3] - m[1]) / s];
	}
	const n = Math.hypot(...q) || 1;
	return [q[0] / n, q[1] / n, q[2] / n, q[3] / n];
}

/** De `a` a `b` por el camino corto, a velocidad constante. */
function slerp(a: Q, b: Q, f: number): Q {
	let d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
	const c: Q = d < 0 ? [-b[0], -b[1], -b[2], -b[3]] : [...b];
	d = Math.abs(d);
	if (d > 0.9995) {
		const q = a.map((x, i) => x + (c[i] - x) * f) as Q;
		const n = Math.hypot(...q) || 1;
		return [q[0] / n, q[1] / n, q[2] / n, q[3] / n];
	}
	const th = Math.acos(d);
	const s = Math.sin(th);
	const ka = Math.sin((1 - f) * th) / s;
	const kb = Math.sin(f * th) / s;
	return [
		a[0] * ka + c[0] * kb,
		a[1] * ka + c[1] * kb,
		a[2] * ka + c[2] * kb,
		a[3] * ka + c[3] * kb,
	];
}

const multiplicar = (a: M3, b: M3): M3 => {
	const r: M3 = [0, 0, 0, 0, 0, 0, 0, 0, 0];
	for (let i = 0; i < 3; i++)
		for (let j = 0; j < 3; j++)
			r[i * 3 + j] = a[i * 3] * b[j] + a[i * 3 + 1] * b[3 + j] + a[i * 3 + 2] * b[6 + j];
	return r;
};

const giroX = (a: number): M3 => {
	const c = Math.cos(a);
	const s = Math.sin(a);
	return [1, 0, 0, 0, c, -s, 0, s, c];
};

/** El giro final: la cara del resultado de frente (`geometria.giroDeCara`), inclinada. */
function giroFinal(p: PiezaTirada, ladeo: number): Q {
	const m = giroDeCara(geometria(tipoDe(p) ?? 6), caraDelResultado(p), ladeo);
	return aCuaternion(multiplicar(giroX(INCLINACION), m));
}

/** La luz viene de arriba a la izquierda y de delante (Y arriba, Z hacia quien mira). */
const LUZ = (() => {
	const l = [-0.35, 0.55, 1];
	const n = Math.hypot(...l);
	return [l[0] / n, l[1] / n, l[2] / n];
})();

/** Lo que hace falta de cada cara para pintarla en cada fotograma. */
type CaraPintada = {
	n: [number, number, number];
	u: [number, number, number];
	w: [number, number, number];
	c: [number, number, number];
};

/**
 * Pone el giro `m` a las caras de un dado ya montado en el SVG (`data-cara`):
 * esconde las que no miran a quien juega y a las demás les da su
 * transformación afín y su sombra.
 */
function pintarDado(svg: SVGSVGElement, caras: CaraPintada[], m: M3) {
	const g = (v: number[]) => [
		m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
		m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
		m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
	];
	const grupos = svg.querySelectorAll<SVGGElement>("[data-cara]");
	caras.forEach((cara, i) => {
		const el = grupos[i];
		if (!el) return;
		const n = g(cara.n);
		if (n[2] <= 0.02) {
			el.setAttribute("display", "none");
			return;
		}
		const w = g(cara.w);
		const u = g(cara.u);
		const c = g(cara.c);
		// En la cara, X va por `w` e Y (hacia abajo, como en SVG) por −`u`; en
		// la pantalla, Y va hacia abajo y la geometría hacia arriba.
		el.setAttribute(
			"transform",
			`matrix(${w[0]},${-w[1]},${-u[0]},${u[1]},${c[0] * ESCALA},${-c[1] * ESCALA})`,
		);
		el.removeAttribute("display");
		const luz = Math.max(0, n[0] * LUZ[0] + n[1] * LUZ[1] + n[2] * LUZ[2]);
		el.querySelector("[data-luz]")?.setAttribute("fill-opacity", String(0.5 * (1 - luz)));
	});
}

/** La moneda girada `fi` radianes sobre X (con la inclinación ya sumada). */
function pintarMoneda(svg: SVGSVGElement, fi: number) {
	const cos = Math.cos(fi);
	const dy = (Math.sin(fi) * GROSOR) / 2;
	const delante = cos >= 0;
	const [cara, cruz, canto, trasera] = ["cara", "cruz", "canto", "trasera"].map((k) =>
		svg.querySelector<SVGElement>(`[data-moneda="${k}"]`),
	);
	// La cara que se ve, con su centro adelantado medio grosor; detrás, el canto.
	const visible = delante ? cara : cruz;
	const oculta = delante ? cruz : cara;
	const signo = delante ? 1 : -1;
	visible?.setAttribute("transform", `matrix(1,0,0,${Math.abs(cos)},0,${signo * dy})`);
	visible?.removeAttribute("display");
	oculta?.setAttribute("display", "none");
	trasera?.setAttribute(
		"transform",
		`matrix(1,0,0,${Math.max(Math.abs(cos), 0.001)},0,${-signo * dy})`,
	);
	const r = MONEDA / 2;
	canto?.setAttribute("x", String(-r));
	canto?.setAttribute("y", String(-Math.abs(dy)));
	canto?.setAttribute("height", String(Math.abs(dy) * 2));
	// Un poco más oscura cuanto más de canto.
	visible
		?.querySelector("[data-luz]")
		?.setAttribute("fill-opacity", String(0.3 * (1 - Math.abs(cos))));
}

/** Un dado montado: todas sus caras en SVG; las coloca `pintarDado`. */
function Dado({ p, estilo, fondo }: { p: PiezaTirada; estilo: Estilo; fondo: string }) {
	const id = useId();
	const t = tipoDe(p) ?? 6;
	const g = geometria(t);
	const radio = Math.max(...g.vertices.map((v) => Math.hypot(...v))) * ESCALA;
	const tinta = { fill: estilo.numeros, stroke: estilo.contorno, paintOrder: "stroke" as const };
	return (
		<svg
			className="absolute overflow-visible"
			style={{ left: -radio, top: -radio, width: radio * 2, height: radio * 2 }}
			viewBox={`${-radio} ${-radio} ${radio * 2} ${radio * 2}`}
			opacity={estilo.acabado === "translucido" && !estilo.imagen ? 0.88 : undefined}
			aria-hidden
		>
			{g.caras.map((_, i) => {
				const pl = planoCara(g, i);
				const d = pl.radio * 2 * ESCALA;
				const puntos = pl.puntos.map(([x, y]) => `${x * ESCALA},${-y * ESCALA}`).join(" ");
				// El trozo del fondo de esta cara, como en el 3D (`escena3d.ts::mallaDado`): un
				// cuadrado centrado en la cara, con la misma escala en los dos ejes, en el que
				// el radio de la cara es el 46 % del lado. Estirado a la caja del polígono, una
				// imagen propia salía aplastada en los triángulos (d4, d8, d20).
				const [fx, fy, fl] = trozoDeCara(estilo, i);
				const celda = (pl.radio * ESCALA) / 0.46;
				const escala = celda / fl;
				const recorte = `${id}-${i}`;
				let dentro: React.ReactNode = null;
				if (p.forma === "planar") {
					const s = valoresSimbolo(g.valores[i]);
					if (s)
						dentro = (
							<path
								d={s}
								transform={`translate(${-0.31 * d},${-0.31 * d}) scale(${(0.62 * d) / 100})`}
								strokeWidth={5}
								{...tinta}
							/>
						);
				} else if (p.forma === "dado" && t === 4) {
					const esquinas = esquinasD4(g, g.valores, i);
					const tam = d * 0.17;
					dentro = pl.puntos.map(([x, y], k) => (
						<text
							key={k}
							x={0}
							y={0}
							transform={`translate(${x * ESCALA * 0.5},${-y * ESCALA * 0.5}) rotate(${(Math.atan2(x, y) * 180) / Math.PI})`}
							fontSize={tam}
							strokeWidth={tam * 0.16}
							{...tinta}
							className="font-bold"
							textAnchor="middle"
							dominantBaseline="central"
						>
							{esquinas[k]}
						</text>
					));
				} else if (p.forma === "dado") {
					const texto = textoCara(t, g.valores[i], p.decenas);
					const tam =
						d * (t === 10 ? 0.2 : t >= 12 || t === 8 ? 0.26 : 0.34) * (texto.length > 2 ? 0.78 : 1);
					dentro = (
						<text
							x={0}
							y={0}
							fontSize={tam}
							strokeWidth={tam * 0.14}
							{...tinta}
							className="font-bold"
							textAnchor="middle"
							dominantBaseline="central"
						>
							{texto}
						</text>
					);
				}
				return (
					<g key={i} data-cara={i} display="none">
						<clipPath id={recorte}>
							<polygon points={puntos} />
						</clipPath>
						{/* Con el borde del mismo color: sin él se verían rendijas entre caras. */}
						<polygon
							points={puntos}
							fill={estilo.color}
							stroke={estilo.color}
							strokeWidth={1.2}
							strokeLinejoin="round"
						/>
						<image
							href={fondo}
							x={-celda / 2 - fx * escala}
							y={-celda / 2 - fy * escala}
							width={LADO_FONDO * escala}
							height={LADO_FONDO * escala}
							preserveAspectRatio="none"
							clipPath={`url(#${CSS.escape(recorte)})`}
						/>
						<polygon
							data-luz
							points={puntos}
							fill="#000"
							fillOpacity={0}
							stroke="rgba(0,0,0,.28)"
							strokeWidth={0.8}
							strokeLinejoin="round"
						/>
						{dentro}
					</g>
				);
			})}
		</svg>
	);
}

/** El símbolo de cada cara del dado planar (1 = planeswalk, 6 = caos). */
const valoresSimbolo = (v: number) => (v === 1 ? PLANESWALK : v === 6 ? CAOS : null);

/** La moneda: dos caras y el canto entre ellas; la coloca `pintarMoneda`. */
function Moneda({ estilo }: { estilo: Estilo }) {
	const id = useId();
	const metal = METALES[estilo.moneda];
	const r = MONEDA / 2;
	const brillo = `${id}-brillo`;
	const cara = (k: "cara" | "cruz", simbolo: string) => (
		<g data-moneda={k} display="none">
			<circle r={r} fill={`url(#${CSS.escape(brillo)})`} stroke={metal.borde} strokeWidth={3} />
			<circle r={r - 8} fill="none" stroke={metal.borde} strokeOpacity={0.35} strokeWidth={1.5} />
			<path
				d={simbolo}
				transform={`translate(${-0.31 * MONEDA},${-0.31 * MONEDA}) scale(${(0.62 * MONEDA) / 100})`}
				fill={metal.simbolo}
				stroke={metal.trazo}
				strokeWidth={5}
				paintOrder="stroke"
			/>
			<circle data-luz r={r} fill="#000" fillOpacity={0} />
		</g>
	);
	return (
		<svg
			className="absolute overflow-visible"
			style={{ left: -r, top: -r, width: MONEDA, height: MONEDA }}
			viewBox={`${-r} ${-r} ${MONEDA} ${MONEDA}`}
			aria-hidden
		>
			<defs>
				<radialGradient id={brillo} cx="40%" cy="35%" r="70%">
					<stop offset="0" stopColor={metal.claro} />
					<stop offset="1" stopColor={metal.oscuro} />
				</radialGradient>
			</defs>
			<ellipse data-moneda="trasera" rx={r} ry={r} fill={metal.oscuro} />
			<rect data-moneda="canto" width={MONEDA} fill={metal.canto} />
			{cara("cara", CARA)}
			{cara("cruz", CRUZ)}
		</svg>
	);
}

/**
 * Una tirada en «3D ligero»: las piezas entran desde abajo (el lado de quien
 * tira), dan tumbos y se quedan en el centro de la mesa.
 */
export function DadosCss({
	tirada,
	estilo,
	fondo,
	alAcabar,
	sonido = true,
}: {
	tirada: Tirada;
	estilo: Estilo;
	/** El fondo de las caras (`data:` URL de `texturas.urlFondo`). */
	fondo: string;
	alAcabar: () => void;
	sonido?: boolean;
}) {
	const caja = useRef<HTMLDivElement>(null);
	const acabar = useRef(alAcabar);
	acabar.current = alAcabar;
	const piezas = useMemo(() => {
		const r = generador(tirada.semilla);
		return tirada.piezas.map((p, i, todas) => {
			const porFila = Math.min(8, todas.length);
			const fila = Math.floor(i / porFila);
			const col = i % porFila;
			const t = tipoDe(p);
			return {
				p,
				fin: {
					x: (col - (porFila - 1) / 2) * 108 + entre(r, -12, 12),
					y: fila * 108 - 40 + entre(r, -12, 12),
				},
				inicio: { x: entre(r, -120, 120), y: 340 },
				giros: [giroAlAzar(r), giroAlAzar(r), giroAlAzar(r)] as Q[],
				vueltas: 3 + Math.floor(r() * 3),
				azarSonido: r(),
				final: p.forma === "moneda" ? null : giroFinal(p, entre(r, -0.25, 0.25)),
				caras:
					t === null
						? []
						: geometria(t).caras.map((_, k) => {
								const pl = planoCara(geometria(t), k);
								return { n: pl.n, u: pl.u, w: pl.w, c: pl.c } as CaraPintada;
							}),
			};
		});
	}, [tirada.piezas, tirada.semilla]);

	useEffect(() => {
		const raiz = caja.current;
		if (!raiz) return;
		const nodos = piezas.map((_, i) => ({
			lugar: raiz.querySelector<HTMLElement>(`[data-lugar="${i}"]`),
			sombra: raiz.querySelector<HTMLElement>(`[data-sombra="${i}"]`),
			svg: raiz.querySelector<SVGSVGElement>(`[data-lugar="${i}"] svg`),
		}));
		const retraso = (i: number) => i * 45;
		const total = DURA + retraso(piezas.length - 1);

		/** Pinta cada pieza a `ms` del principio de la tirada. */
		const pintar = (ms: number) => {
			piezas.forEach((x, i) => {
				const { lugar, sombra, svg } = nodos[i];
				if (!lugar || !sombra || !svg) return;
				const tiempo = Math.min(1, Math.max(0, (ms - retraso(i)) / DURA));
				const [k, f] = tramo(avance(tiempo));
				const px = x.inicio.x + (x.fin.x - x.inicio.x) * mezcla(CAMINO, k, f);
				const py = x.inicio.y + (x.fin.y - x.inicio.y) * mezcla(CAMINO, k, f);
				const alto = mezcla(ALTO, k, f);
				lugar.style.transform = `translate(${px}px,${py - alto}px) scale(${1 + alto / 260})`;
				sombra.style.transform = `translate(${px + 8}px,${py + 10}px) scale(${1 + alto / 120})`;
				sombra.style.opacity = String(0.45 - alto / 300);
				if (x.final === null) {
					// La moneda da vueltas sobre su eje y acaba de cara o de cruz.
					const vueltas =
						(x.vueltas * 360 + (x.p.forma === "moneda" && x.p.cara ? 0 : 180)) *
						mezcla([0, 0.62, 0.86, 1, 1], k, f);
					pintarMoneda(svg, (vueltas * Math.PI) / 180 + INCLINACION);
				} else {
					const claves = [...x.giros, x.final, x.final];
					pintarDado(svg, x.caras, deCuaternion(slerp(claves[k], claves[k + 1], f)));
				}
			});
		};

		pintar(0);
		piezas.forEach((x, i) => {
			if (!sonido) return;
			const moneda = x.p.forma === "moneda";
			if (moneda && i === 0) lanzarMoneda();
			for (const [m, fuerza] of GOLPES) {
				const s = (retraso(i) + tiempoDe(MOMENTOS[m]) * DURA) / 1000;
				if (moneda) golpeMoneda(fuerza, s);
				else golpeDado(fuerza * (0.8 + 0.4 * x.azarSonido), s);
			}
		});
		let vivo = true;
		let cuadro = 0;
		const empiece = performance.now();
		const paso = (ahora: number) => {
			if (!vivo) return;
			const ms = ahora - empiece;
			pintar(ms);
			if (ms < total) cuadro = requestAnimationFrame(paso);
			else acabar.current();
		};
		cuadro = requestAnimationFrame(paso);
		return () => {
			vivo = false;
			cancelAnimationFrame(cuadro);
		};
	}, [piezas, sonido]);

	return (
		<div ref={caja} className="absolute inset-0 flex items-center justify-center">
			{piezas.map((_, i) => (
				<div key={`s${i}`} data-sombra={i} className="absolute size-0 opacity-0">
					<div className="absolute -top-8 -left-10 h-16 w-20 rounded-full bg-black/60 blur-md" />
				</div>
			))}
			{piezas.map((x, i) => (
				<div key={i} data-lugar={i} className="absolute size-0">
					{x.p.forma === "moneda" ? (
						<Moneda estilo={estilo} />
					) : (
						<Dado p={x.p} estilo={estilo} fondo={fondo} />
					)}
				</div>
			))}
		</div>
	);
}
