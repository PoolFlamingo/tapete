/**
 * **El código de partida pegado.** Una invitación llega como código suelto o como enlace entero
 * (`tapete://unirse/<código>`, `proxybuilder://unirse/<código>` o el de la web,
 * `https://tapete.zodhub.app/join/<código>`); el campo de «Unirse con un
 * código» acepta los dos. Rust sigue validando el código: esto solo quita lo que sobra para que el
 * campo no lo rechace.
 */

/** El principio de un enlace de invitación: cualquiera de los dos esquemas o la web. */
const INICIO_ENLACE =
	/^(?:(?:proxybuilder|tapete):\/\/unirse\/|https?:\/\/(?:www\.)?tapete\.zodhub\.app\/join\/)/i;

/** El código de partida directa tal como se pega: sin espacios ni el principio del enlace. */
export function limpiarCodigoDirecto(texto: string): string {
	const sin = texto
		.replace(/\s+/g, "")
		.replace(INICIO_ENLACE, "")
		.replace(/[/?#].*$/, "");
	// El prefijo va en mayúsculas; el resto es base64 y distingue mayúsculas: no se toca.
	return sin.replace(/^pb1-/i, "PB1-");
}
