import { useEffect, useState } from "react";
import { create } from "zustand";
import type { AmigoVista } from "@/bindings/AmigoVista";
import type { EstadoAmigos } from "@/bindings/EstadoAmigos";
import { call, on } from "@/lib/ipc";

/**
 * **Amigos y mensajes** en pantalla (plan Tapete, Fase D). Todo lo decide Rust (`amigos/`):
 * aquí solo se guarda lo último que dijo (`amigos://estado`), qué conversación está abierta y
 * el código de una invitación que hay que llevar a «Unirse con un código».
 */
type AmigosStore = {
	estado: EstadoAmigos | null;
	/** El diálogo de amigos está abierto. */
	abierto: boolean;
	/** La clave del amigo cuya conversación se ve (null: la lista). */
	chat: string | null;
	/** Una invitación aceptada: «Jugar» abre «Jugar en red» con este código y se une con el deck
	 * elegido (si no hay ninguno, el código queda puesto y se pide uno). */
	unirse: string | null;
	/** «Invitar a una partida» sin partida creada: «Jugar» la crea y le invita (su clave). */
	invitarA: string | null;
	/** Un enlace de amigo: «Añadir con un código» sale con este código puesto. */
	anadir: string | null;
	/** Quién me está escribiendo: su clave → hasta cuándo se enseña (ms). */
	escribiendo: Record<string, number>;
	poner: (e: EstadoAmigos) => void;
	abrir: (chat?: string | null) => void;
	cerrar: () => void;
	verChat: (clave: string | null) => void;
	pedirUnirse: (codigo: string | null) => void;
	pedirInvitar: (clave: string | null) => void;
	pedirAnadir: (codigo: string | null) => void;
};

export const useAmigos = create<AmigosStore>((set) => ({
	estado: null,
	abierto: false,
	chat: null,
	unirse: null,
	invitarA: null,
	anadir: null,
	escribiendo: {},
	poner: (estado) => set({ estado }),
	abrir: (chat = null) => set({ abierto: true, chat }),
	cerrar: () => set({ abierto: false, chat: null }),
	verChat: (chat) => set({ chat }),
	pedirUnirse: (unirse) => set({ unirse }),
	pedirInvitar: (invitarA) => set({ invitarA }),
	pedirAnadir: (anadir) => set({ anadir }),
}));

/** Cuánto dura «escribiendo…» desde el último aviso (la otra app lo manda cada 3 s). */
export const ESCRIBIENDO_MS = 6000;

/** El nombre con el que veo a un amigo: el que le puse yo o, si no, el suyo. */
export function nombreAmigo(a: Pick<AmigoVista, "alias" | "apodo">): string {
	return a.alias ?? a.apodo;
}

/** Si ese amigo me está escribiendo ahora (se apaga solo al pasar `ESCRIBIENDO_MS`). */
export function useEscribiendo(clave: string): boolean {
	const hasta = useAmigos((s) => s.escribiendo[clave] ?? 0);
	const [, repintar] = useState(0);
	useEffect(() => {
		const queda = hasta - Date.now();
		if (queda <= 0) return;
		const t = setTimeout(() => repintar((n) => n + 1), queda + 50);
		return () => clearTimeout(t);
	}, [hasta]);
	return hasta > Date.now();
}

function anotarEscribiendo(clave: string, hasta: number | null): void {
	useAmigos.setState((s) => {
		const escribiendo = { ...s.escribiendo };
		if (hasta === null) delete escribiendo[clave];
		else escribiendo[clave] = hasta;
		return { escribiendo };
	});
}

/** Lo que hay que mirar: mensajes sin leer y solicitudes recibidas. */
export function pendientes(e: EstadoAmigos | null): number {
	if (!e?.activo) return 0;
	return e.amigos.reduce((n, a) => n + a.sinLeer, 0) + e.recibidas.length;
}

let iniciado = false;

/** Carga el estado y escucha los cambios (una vez, al montar el shell). */
export function iniciarAmigos(): void {
	if (iniciado) return;
	iniciado = true;
	void call("amigos_estado")
		.then((e) => useAmigos.getState().poner(e))
		.catch(() => {});
	void on("amigos://estado", (e) => useAmigos.getState().poner(e));
	void on("amigos://escribiendo", (clave) => anotarEscribiendo(clave, Date.now() + ESCRIBIENDO_MS));
	// Llegó lo que escribía: ya no está escribiendo.
	void on("amigos://mensajes", (claves) => {
		for (const c of claves) anotarEscribiendo(c, null);
	});
}

/** Solo para los tests. */
export function reiniciarAmigos(): void {
	iniciado = false;
	useAmigos.setState({
		estado: null,
		abierto: false,
		chat: null,
		unirse: null,
		invitarA: null,
		anadir: null,
		escribiendo: {},
	});
}
