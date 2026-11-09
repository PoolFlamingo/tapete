import type { Lang } from "@/lib/idiomas";

/**
 * Las traducciones que trae `@emoji-mart/data/i18n` (1.2.1). De los 15 de la
 * app faltan el rumano (va en inglés) y el chino tradicional (el chino de
 * emoji-mart es el simplificado: se lee igual en los dos).
 */
export type IdiomaSelector =
	| "es"
	| "en"
	| "fr"
	| "de"
	| "it"
	| "pt"
	| "pl"
	| "nl"
	| "ru"
	| "uk"
	| "ja"
	| "ko"
	| "zh";

export function idiomaDelSelector(lang: Lang): IdiomaSelector {
	switch (lang) {
		case "zhs":
		case "zht":
			return "zh";
		case "ro":
			return "en";
		default:
			return lang;
	}
}

/**
 * La traducción del selector, del paquete (cada una en su trozo). 🛑 Hay que
 * pasarla siempre, también en inglés: sin ella, emoji-mart la pide a su CDN.
 */
export async function traduccionDelSelector(idioma: IdiomaSelector): Promise<unknown> {
	const m = await (() => {
		switch (idioma) {
			case "es":
				return import("@emoji-mart/data/i18n/es.json");
			case "fr":
				return import("@emoji-mart/data/i18n/fr.json");
			case "de":
				return import("@emoji-mart/data/i18n/de.json");
			case "it":
				return import("@emoji-mart/data/i18n/it.json");
			case "pt":
				return import("@emoji-mart/data/i18n/pt.json");
			case "pl":
				return import("@emoji-mart/data/i18n/pl.json");
			case "nl":
				return import("@emoji-mart/data/i18n/nl.json");
			case "ru":
				return import("@emoji-mart/data/i18n/ru.json");
			case "uk":
				return import("@emoji-mart/data/i18n/uk.json");
			case "ja":
				return import("@emoji-mart/data/i18n/ja.json");
			case "ko":
				return import("@emoji-mart/data/i18n/ko.json");
			case "zh":
				return import("@emoji-mart/data/i18n/zh.json");
			default:
				return import("@emoji-mart/data/i18n/en.json");
		}
	})();
	return m.default;
}
