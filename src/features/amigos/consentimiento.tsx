import { useState } from "react";
import { Icono } from "@/components/icono";
import { useLang } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { DialogFooter } from "@/components/ui/dialog";
import { call } from "@/lib/ipc";
import { useAmigos } from "@/stores/amigos-store";

/**
 * **Antes de nada, el consentimiento** (plan D0): la app no habla con el servidor de Tapete hasta
 * que la persona lee qué se manda y acepta. Sin aceptar, todo lo demás de la app sigue igual.
 */
export function Consentimiento({ onError }: { onError: (e: unknown) => void }) {
	const { t } = useLang();
	const [ocupado, setOcupado] = useState(false);
	const aceptar = async () => {
		setOcupado(true);
		try {
			useAmigos.getState().poner(await call("amigos_activar"));
		} catch (e) {
			onError(e);
		} finally {
			setOcupado(false);
		}
	};
	return (
		<div className="flex flex-col gap-2.5 text-xs">
			<p>
				{t(
					"Para tener amigos, escribiros y mandaros invitaciones, la app usa el servidor de Tapete. La partida sigue yendo de un equipo a otro, sin pasar por él.",
				)}
			</p>
			<div className="grid gap-2.5 sm:grid-cols-2">
				<div className="flex flex-col gap-1.5 rounded-lg border p-2.5">
					<span className="flex items-center gap-1.5 font-medium">
						<Icono icon="lucide:upload" className="size-4 text-primary" />
						{t("Se manda")}
					</span>
					<ul className="list-disc space-y-1 pl-4 text-muted-foreground">
						<li>{t("Tu apodo (el de jugar en red) y la clave pública de este equipo.")}</li>
						<li>{t("La plataforma y la versión de la app.")}</li>
						<li>{t("Con quién eres amigo y cuándo se te vio conectado.")}</li>
						<li>{t("Tus mensajes, cifrados: el servidor no puede leerlos.")}</li>
					</ul>
				</div>
				<div className="flex flex-col gap-1.5 rounded-lg border p-2.5">
					<span className="flex items-center gap-1.5 font-medium">
						<Icono icon="lucide:shield-check" className="size-4 text-primary" />
						{t("No se manda")}
					</span>
					<ul className="list-disc space-y-1 pl-4 text-muted-foreground">
						<li>{t("El texto de tus mensajes ni las invitaciones.")}</li>
						<li>{t("Tus decks, tus cartas ni tus partidas.")}</li>
						<li>{t("Tu nombre real, tu correo ni tus contactos.")}</li>
					</ul>
				</div>
			</div>
			<p className="text-muted-foreground">
				{t(
					"Los mensajes que nadie recoge se borran a los 30 días. Puedes dejarlo cuando quieras en este mismo diálogo y pedir que se borre todo del servidor.",
				)}
			</p>
			<DialogFooter className="items-center gap-2 sm:justify-between">
				<Button variant="link" size="sm" onClick={() => void call("amigos_privacidad")}>
					<Icono icon="lucide:external-link" className="size-4" />
					{t("Política de privacidad")}
				</Button>
				<Button size="sm" onClick={() => void aceptar()} disabled={ocupado}>
					{ocupado && <Icono icon="lucide:loader-2" className="size-4 animate-spin" />}
					{t("Aceptar y entrar")}
				</Button>
			</DialogFooter>
		</div>
	);
}
