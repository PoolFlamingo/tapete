-- Amigos y mensajes (plan Tapete, Fase D). El servidor solo guarda bytes cifrados y los borra en
-- cuanto se confirman: la conversación, ya descifrada, vive aquí, como el resto de datos de la app.
--
-- `amigo` es la clave pública ed25519 del amigo en hex (su identidad). `mio` = 1 si lo escribí
-- yo. `tipo`: `texto` o `invitacion` (entonces `texto` lleva el código directo de la partida).
-- `enviado`: cuándo se escribió (ms, reloj de quien lo escribió); `orden`: cuándo llegó a este
-- equipo o se escribió en él (ms, reloj de aquí), que es lo que ordena la conversación.
-- `estado`: los míos `pendiente` (aún sin mandar), `enviado` (el servidor lo tiene),
-- `entregado` (el amigo lo recogió) o `fallido`; los recibidos, `recibido`.
-- `id_cliente`: el que pone la app al mandar (el servidor no duplica si se reintenta con él).
-- `id_servidor`: el del servidor (para no guardar dos veces uno recibido si el `ack` se pierde).
CREATE TABLE amigos_mensajes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    amigo       TEXT NOT NULL,
    mio         INTEGER NOT NULL,
    tipo        TEXT NOT NULL,
    texto       TEXT NOT NULL,
    enviado     INTEGER NOT NULL,
    orden       INTEGER NOT NULL,
    estado      TEXT NOT NULL,
    id_cliente  TEXT,
    id_servidor TEXT
);
CREATE INDEX amigos_mensajes_amigo ON amigos_mensajes(amigo, orden);
CREATE UNIQUE INDEX amigos_mensajes_cliente ON amigos_mensajes(id_cliente)
    WHERE id_cliente IS NOT NULL;
CREATE UNIQUE INDEX amigos_mensajes_servidor ON amigos_mensajes(id_servidor)
    WHERE id_servidor IS NOT NULL AND mio = 0;

-- Lo que este equipo sabe de cada amigo: su clave de cifrado tal como la vio la última vez (si
-- cambia, se avisa y se quita «verificado»: plan D3), si se comprobó el número de seguridad, y
-- hasta dónde se leyó la conversación (`leido_hasta`, un `orden`).
CREATE TABLE amigos_claves (
    amigo         TEXT PRIMARY KEY,
    clave_cifrado TEXT NOT NULL,
    verificado    INTEGER NOT NULL DEFAULT 0,
    cambio        INTEGER NOT NULL DEFAULT 0,
    leido_hasta   INTEGER NOT NULL DEFAULT 0
);
