/**
 * **Cuándo se vio a un amigo**, en palabras del idioma («hace 5 minutos»), con
 * `Intl.RelativeTimeFormat`. El servidor da la fecha (ISO); `null` si no se sabe o
 * si la fecha no se entiende.
 */
export function vistoHace(iso: string | null, locale: string, ahora = Date.now()): string | null {
	if (!iso) return null;
	const cuando = Date.parse(iso);
	if (Number.isNaN(cuando)) return null;
	const seg = Math.max(0, Math.round((ahora - cuando) / 1000));
	const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
	if (seg < 60) return rtf.format(0, "second");
	const min = Math.round(seg / 60);
	if (min < 60) return rtf.format(-min, "minute");
	const horas = Math.round(min / 60);
	if (horas < 24) return rtf.format(-horas, "hour");
	const dias = Math.round(horas / 24);
	if (dias < 30) return rtf.format(-dias, "day");
	const meses = Math.round(dias / 30);
	if (meses < 12) return rtf.format(-meses, "month");
	return rtf.format(-Math.round(dias / 365), "year");
}
