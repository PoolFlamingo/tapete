-- Lo que cada uno apunta de un amigo, solo para sí (plan Tapete, Fase D): el nombre con el que lo
-- ve (`alias`) y una nota (`nota`, como la de los amigos del LoL). El amigo no lo ve y el servidor
-- no lo sabe. Aparte de `amigos_claves` porque esa fila nace con la clave de cifrado del amigo, y
-- esto se puede poner antes. `amigo`: su identidad en hex. Una fila sin ninguno de los dos se borra.
CREATE TABLE amigos_alias (
    amigo TEXT PRIMARY KEY,
    alias TEXT,
    nota  TEXT
);
