/**
 * **Meter un texto donde está el cursor** de un campo: sustituye lo
 * seleccionado (o nada, si el cursor está suelto) y dice dónde queda el
 * cursor después. Si no cabe en `maximo` (contado como `maxLength`, en
 * unidades UTF-16), no cambia nada: un emoji cortado por la mitad sería basura.
 */
export function insertarEnCursor(
	texto: string,
	inicio: number | null,
	fin: number | null,
	nuevo: string,
	maximo = Number.POSITIVE_INFINITY,
): { texto: string; cursor: number } | null {
	// Sin selección conocida (el campo nunca tuvo el foco), al final.
	const a = Math.min(Math.max(inicio ?? texto.length, 0), texto.length);
	const b = Math.min(Math.max(fin ?? a, a), texto.length);
	const resultado = texto.slice(0, a) + nuevo + texto.slice(b);
	if (resultado.length > maximo) return null;
	return { texto: resultado, cursor: a + nuevo.length };
}
