import type { AppError } from "@/bindings/AppError";
import type { ErrorCode } from "@/bindings/ErrorCode";
import { paraTraducir } from "@/components/language-provider";

/**
 * El texto de cada código de error de Rust (`src-tauri/src/error.rs`), en
 * español; `t()` lo pasa al idioma de la interfaz. Los `{datos}` salen de
 * `params`.
 */
const TEXTOS: Record<ErrorCode, string> = {
	Network: paraTraducir("No hay conexión o el servidor no responde."),
	RateLimited: paraTraducir("El servicio pide ir más despacio. Vuelve a intentarlo en un momento."),
	NotFound: paraTraducir("No se ha encontrado lo que se buscaba."),
	Io: paraTraducir("No se ha podido leer o escribir en el disco."),
	Db: paraTraducir("La biblioteca local ha dado un error."),
	InvalidInput: paraTraducir("Los datos no son válidos."),
	Cancelled: paraTraducir("Cancelado."),
	Pdf: paraTraducir("No se ha podido generar el PDF."),
	Image: paraTraducir("No se ha podido procesar la imagen."),
	Unsupported: paraTraducir("Ese formato de fichero no se admite."),
	Busy: paraTraducir("Hay un trabajo en marcha; espera a que termine."),
	Internal: paraTraducir("Error interno inesperado."),
	Rejected: paraTraducir("La mesa no deja hacer eso ahora."),
	Server: paraTraducir("El servidor ha dado un error."),
};

/**
 * Los rechazos de la mesa (`mesa::Rechazo`, en `params.motivo`), la
 * actualización que espera a la partida (`Busy` con `motivo: partida`) y los
 * errores del servidor de Tapete (`params.code`), con su texto propio. Lo que
 * no esté aquí sale con el texto general de su código.
 */
const MOTIVOS: Partial<Record<ErrorCode, Record<string, string>>> = {
	Busy: {
		partida: paraTraducir("Hay una partida abierta: termínala o sal de la mesa para actualizar."),
	},
	InvalidInput: {
		falta_lista: paraTraducir(
			"Este deck tiene más cartas de las que se juegan: elige una lista para jugar.",
		),
		lista: paraTraducir("La lista para jugar no vale para este formato: revísala."),
		lista_sin_nombre: paraTraducir("Ponle un nombre a la lista."),
		codigo: paraTraducir("Ese código no es válido: cópialo entero, empieza por PB1-."),
		// El de pasar un deck (`PBD1-…`) pegado donde va el de una partida.
		codigo_de_deck: paraTraducir(
			"Ese es el código de «Recibir un deck» (empieza por PBD1-), no el de una partida.",
		),
		deck: paraTraducir("Elige un deck para jugar."),
		identidad: paraTraducir("Ese fichero no es una identidad de Tapete."),
		// Amigos y mensajes (`commands/amigos.rs`).
		amigos_apagado: paraTraducir("Primero acepta usar amigos."),
		mensaje_largo: paraTraducir("El mensaje es demasiado largo."),
		vacio: paraTraducir("Escribe algo antes de enviarlo."),
		sin_rele: paraTraducir(
			"Esa partida no se puede compartir a distancia todavía: espera unos segundos y vuelve a probar.",
		),
	},
	// Los de `crate::error::{red, estado_http}`: el porqué de un fallo de red.
	Network: {
		conexion: paraTraducir(
			"No se ha podido conectar con el servidor. Revisa la conexión y que el cortafuegos o el antivirus dejen salir a la app.",
		),
		tiempo: paraTraducir("El servidor ha tardado demasiado en responder. Vuelve a intentarlo."),
		respuesta: paraTraducir("El servidor ha mandado una respuesta que la app no entiende."),
		rechazo: paraTraducir(
			"El servidor ha rechazado la petición (error {estado}). Suele ser una VPN, un proxy o la red bloqueada: prueba con otra conexión.",
		),
		estado: paraTraducir(
			"El servidor ha devuelto un error ({estado}). Vuelve a intentarlo más tarde.",
		),
		sin_respuesta: paraTraducir("El anfitrión no responde."),
		desconectado: paraTraducir("Se ha perdido la conexión con la partida."),
	},
	NotFound: {
		sin_sala: paraTraducir("No hay ninguna partida en red abierta."),
		sin_vuelta: paraTraducir("Ya no hay ninguna partida a la que volver."),
	},
	Rejected: {
		NoExiste: paraTraducir("Esa carta o ese jugador ya no está ahí."),
		SinPermiso: paraTraducir("Eso es de otro jugador: no puedes tocarlo."),
		ZonaVacia: paraTraducir("No queda ninguna carta ahí."),
		FueraDeTurno: paraTraducir("Solo el jugador activo puede hacer eso."),
		Limite: paraTraducir("Es demasiado (un número o un texto pasa del límite)."),
		NoSePuede: paraTraducir("Eso no se puede hacer ahora."),
		codigo: paraTraducir("El código no es el de esta partida: pide uno nuevo al anfitrión."),
		version: paraTraducir("Tenéis versiones distintas de la app: actualizad los dos."),
		llena: paraTraducir("La partida está llena."),
		empezada: paraTraducir("La partida ya ha empezado: puedes entrar a mirar."),
		sin_deck: paraTraducir("La partida empezó sin ti: no habías elegido deck."),
		ritmo: paraTraducir("Demasiadas acciones seguidas: espera un momento."),
		espectador: paraTraducir("Estás mirando: los espectadores no pueden jugar."),
		sin_partida: paraTraducir("La partida aún no ha empezado."),
		expulsado: paraTraducir("El anfitrión te ha expulsado de la partida."),
		no_anfitrion: paraTraducir("Solo el anfitrión puede hacer eso."),
		sin_mayoria: paraTraducir(
			"Sin la mayoría de los jugadores conectados no se juega: espera a que vuelvan.",
		),
		no_activo: paraTraducir("Ese anfitrión aún no tiene la mayoría de los jugadores."),
		partida_abierta: paraTraducir(
			"Hay una partida abierta: sal de la mesa para ver una repetición.",
		),
		sin_registro: paraTraducir("Esta partida no se guardó entera: no se puede repetir."),
		sin_terminar: paraTraducir("La partida aún no ha terminado."),
		sala_cambio: paraTraducir(
			"La sala ha cambiado mientras elegías: revisa los jugadores y vuelve a empezar.",
		),
		ajeno: paraTraducir("En red cada uno cambia solo su propio mazo."),
		sin_mazos: paraTraducir(
			"Tras el traspaso del anfitrión no se puede abrir otra partida del encuentro: ya no están los mazos de todos.",
		),
		cambio: paraTraducir("Ese cambio de banquillo no encaja: revisa las cartas."),
		repeticion: paraTraducir("Estás viendo una repetición: no se puede jugar."),
	},
	Unsupported: {
		version: paraTraducir(
			"Esta partida es de una versión más nueva de la app: actualiza para verla.",
		),
		// Android: se ha abierto su ajuste de «instalar apps desconocidas»
		// (`android::instalar_apk`).
		instalar_permiso: paraTraducir(
			"Android necesita tu permiso para instalar la actualización: se ha abierto su ajuste. Permite instalar apps desde Tapete, vuelve y pulsa «Instalar» otra vez.",
		),
	},
	// Los códigos del servidor de Tapete (`params.code`; `docs/api.md` del servidor).
	Server: {
		bad_request: paraTraducir("El servidor no ha entendido la petición."),
		payload_too_large: paraTraducir("El mensaje es demasiado largo."),
		message_too_large: paraTraducir("El mensaje es demasiado largo."),
		missing_signature: paraTraducir("El servidor no ha reconocido esta instalación."),
		bad_key: paraTraducir("El servidor no ha reconocido esta instalación."),
		bad_timestamp: paraTraducir("El servidor no ha reconocido esta instalación."),
		bad_nonce: paraTraducir("El servidor no ha reconocido esta instalación."),
		bad_signature: paraTraducir("El servidor no ha reconocido esta instalación."),
		replayed_nonce: paraTraducir("El servidor no ha reconocido esta instalación."),
		clock_skew: paraTraducir(
			"La hora de este equipo no es correcta: ajústala (o pon la hora automática) y vuelve a probar.",
		),
		not_registered: paraTraducir("Este equipo ya no está dado de alta en el servidor."),
		player_blocked: paraTraducir("El servidor ha bloqueado esta instalación."),
		app_outdated: paraTraducir("Esta versión de la app ya no vale para amigos: actualízala."),
		rate_limited: paraTraducir("Demasiado seguido: espera un momento y vuelve a probar."),
		too_many_connections: paraTraducir("Demasiado seguido: espera un momento y vuelve a probar."),
		bad_encryption_key: paraTraducir(
			"La clave de cifrado de esa persona no está bien firmada: no se le puede escribir.",
		),
		friend_code_not_found: paraTraducir("Ese código de amigo no existe."),
		cannot_befriend_self: paraTraducir("Ese es tu propio código."),
		already_friends: paraTraducir("Ya sois amigos."),
		blocked_by_you: paraTraducir("Tienes bloqueada a esa persona: desbloquéala primero."),
		daily_request_limit: paraTraducir("Has pedido demasiadas amistades hoy: prueba mañana."),
		request_not_found: paraTraducir("Esa solicitud ya no existe."),
		not_friends: paraTraducir("Ya no sois amigos: no se le puede escribir."),
		recipient_inbox_full: paraTraducir(
			"Esa persona tiene demasiados mensajes sin leer: espera a que los lea.",
		),
		daily_report_limit: paraTraducir("Has enviado demasiadas denuncias hoy."),
		internal: paraTraducir("El servidor ha tenido un problema: prueba más tarde."),
		maintenance: paraTraducir("El servidor está en mantenimiento: prueba en unos minutos."),
	},
};

/** ¿Es un `AppError` de Rust? (Lo que llega del `invoke` es `unknown`.) */
export function esAppError(e: unknown): e is AppError {
	if (typeof e !== "object" || e === null) return false;
	const code = (e as { code?: unknown }).code;
	return typeof code === "string" && code in TEXTOS;
}

/** Cualquier error, en una frase para la persona. */
export function mensajeDeError(
	e: unknown,
	t: (es: string, vars?: Record<string, string | number>) => string,
): string {
	if (esAppError(e)) {
		const clave = e.params.motivo ?? e.params.code;
		const propio = clave ? MOTIVOS[e.code]?.[clave] : undefined;
		return t(propio ?? TEXTOS[e.code], e.params);
	}
	if (e instanceof Error) return e.message;
	return String(e);
}
