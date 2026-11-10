import { create } from "zustand";
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
	/** Una invitación aceptada: «Jugar» abre «Jugar en red» con este código puesto. */
	unirse: string | null;
	/** Un enlace de amigo: «Añadir con un código» sale con este código puesto. */
	anadir: string | null;
	poner: (e: EstadoAmigos) => void;
	abrir: (chat?: string | null) => void;
	cerrar: () => void;
	verChat: (clave: string | null) => void;
	pedirUnirse: (codigo: string | null) => void;
	pedirAnadir: (codigo: string | null) => void;
};

export const useAmigos = create<AmigosStore>((set) => ({
	estado: null,
	abierto: false,
	chat: null,
	unirse: null,
	anadir: null,
	poner: (estado) => set({ estado }),
	abrir: (chat = null) => set({ abierto: true, chat }),
	cerrar: () => set({ abierto: false, chat: null }),
	verChat: (chat) => set({ chat }),
	pedirUnirse: (unirse) => set({ unirse }),
	pedirAnadir: (anadir) => set({ anadir }),
}));

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
}

/** Solo para los tests. */
export function reiniciarAmigos(): void {
	iniciado = false;
	useAmigos.setState({ estado: null, abierto: false, chat: null, unirse: null, anadir: null });
}
