//! **La conversación con cada amigo**, ya descifrada, y lo que este equipo sabe de sus claves
//! (migración 0010). El servidor no guarda nada de esto en claro.

use std::collections::BTreeMap;

use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use super::db::Db;
use crate::amigos::cifrado::{Carta, Contenido};
use crate::error::AppResult;

/// Un mensaje de la conversación, como lo pinta la pantalla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MensajeAmigo")]
pub struct Mensaje {
    #[ts(type = "number")]
    pub id: i64,
    pub amigo: String,
    pub mio: bool,
    /// `texto` o `invitacion` (entonces `texto` es el código directo de la partida).
    pub tipo: String,
    pub texto: String,
    #[ts(type = "number")]
    pub enviado: i64,
    #[ts(type = "number")]
    pub orden: i64,
    /// `pendiente` · `enviado` · `entregado` · `fallido` (los míos) · `recibido`.
    pub estado: String,
}

fn partes(c: &Carta) -> (&'static str, &str) {
    match &c.contenido {
        Contenido::Texto { texto } => ("texto", texto),
        Contenido::Invitacion { codigo } => ("invitacion", codigo),
    }
}

/// Apunta uno recibido. `false` si ya estaba (el `ack` anterior se perdió).
pub fn guardar_recibido(
    db: &Db,
    amigo: &str,
    carta: &Carta,
    id_servidor: &str,
    orden: i64,
) -> AppResult<bool> {
    let (tipo, texto) = partes(carta);
    let n = db.conn()?.execute(
        "INSERT OR IGNORE INTO amigos_mensajes
             (amigo, mio, tipo, texto, enviado, orden, estado, id_servidor)
         VALUES (?1, 0, ?2, ?3, ?4, ?5, 'recibido', ?6)",
        (amigo, tipo, texto, carta.enviado, orden, id_servidor),
    )?;
    Ok(n > 0)
}

/// Apunta uno mío antes de mandarlo (así no se pierde si el envío falla).
pub fn guardar_mio(
    db: &Db,
    amigo: &str,
    carta: &Carta,
    id_cliente: &str,
    orden: i64,
) -> AppResult<i64> {
    let (tipo, texto) = partes(carta);
    let c = db.conn()?;
    c.execute(
        "INSERT INTO amigos_mensajes
             (amigo, mio, tipo, texto, enviado, orden, estado, id_cliente)
         VALUES (?1, 1, ?2, ?3, ?4, ?5, 'pendiente', ?6)",
        (amigo, tipo, texto, carta.enviado, orden, id_cliente),
    )?;
    Ok(c.last_insert_rowid())
}

pub fn poner_estado(db: &Db, id_cliente: &str, estado: &str) -> AppResult<()> {
    db.conn()?.execute(
        "UPDATE amigos_mensajes SET estado = ?2 WHERE id_cliente = ?1 AND mio = 1
             AND estado != 'entregado'",
        (id_cliente, estado),
    )?;
    Ok(())
}

pub fn enviado(db: &Db, id_cliente: &str, id_servidor: &str) -> AppResult<()> {
    db.conn()?.execute(
        "UPDATE amigos_mensajes SET estado = 'enviado', id_servidor = ?2
         WHERE id_cliente = ?1 AND mio = 1 AND estado != 'entregado'",
        (id_cliente, id_servidor),
    )?;
    Ok(())
}

/// Un mensaje mío por su id local (para reintentar uno fallido).
pub fn mio(db: &Db, id: i64) -> AppResult<Option<(Mensaje, String)>> {
    Ok(db
        .conn()?
        .query_row(
            "SELECT id, amigo, mio, tipo, texto, enviado, orden, estado, id_cliente
             FROM amigos_mensajes WHERE id = ?1 AND mio = 1",
            [id],
            |r| Ok((fila(r)?, r.get::<_, Option<String>>(8)?.unwrap_or_default())),
        )
        .optional()?)
}

/// Los míos que aún no llegaron al servidor (un fallo de red): se reintentan con el mismo
/// `id_cliente` (el servidor no duplica).
pub fn pendientes(db: &Db) -> AppResult<Vec<(Mensaje, String)>> {
    let c = db.conn()?;
    let mut st = c.prepare(
        "SELECT id, amigo, mio, tipo, texto, enviado, orden, estado, id_cliente
         FROM amigos_mensajes WHERE mio = 1 AND estado = 'pendiente' ORDER BY orden LIMIT 50",
    )?;
    let v = st
        .query_map([], |r| {
            Ok((fila(r)?, r.get::<_, Option<String>>(8)?.unwrap_or_default()))
        })?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

fn fila(r: &rusqlite::Row<'_>) -> rusqlite::Result<Mensaje> {
    Ok(Mensaje {
        id: r.get(0)?,
        amigo: r.get(1)?,
        mio: r.get::<_, i64>(2)? != 0,
        tipo: r.get(3)?,
        texto: r.get(4)?,
        enviado: r.get(5)?,
        orden: r.get(6)?,
        estado: r.get(7)?,
    })
}

/// La conversación con un amigo, del más viejo al más nuevo: los `limite` anteriores a
/// `antes` (un id), o los últimos.
pub fn conversacion(
    db: &Db,
    amigo: &str,
    antes: Option<i64>,
    limite: u32,
) -> AppResult<Vec<Mensaje>> {
    let c = db.conn()?;
    let mut st = c.prepare(
        "SELECT id, amigo, mio, tipo, texto, enviado, orden, estado FROM amigos_mensajes
         WHERE amigo = ?1 AND (?2 IS NULL OR id < ?2)
         ORDER BY orden DESC, id DESC LIMIT ?3",
    )?;
    let mut v: Vec<Mensaje> = st
        .query_map((amigo, antes, limite), fila)?
        .collect::<Result<_, _>>()?;
    v.reverse();
    Ok(v)
}

/// Cuántos mensajes sin leer hay de cada amigo.
pub fn no_leidos(db: &Db) -> AppResult<BTreeMap<String, u32>> {
    let c = db.conn()?;
    let mut st = c.prepare(
        "SELECT m.amigo, COUNT(*) FROM amigos_mensajes m
         LEFT JOIN amigos_claves k ON k.amigo = m.amigo
         WHERE m.mio = 0 AND m.orden > COALESCE(k.leido_hasta, 0)
         GROUP BY m.amigo",
    )?;
    let v = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

pub fn marcar_leido(db: &Db, amigo: &str) -> AppResult<()> {
    db.conn()?.execute(
        "UPDATE amigos_claves SET leido_hasta =
             (SELECT COALESCE(MAX(orden), 0) FROM amigos_mensajes WHERE amigo = ?1)
         WHERE amigo = ?1",
        [amigo],
    )?;
    Ok(())
}

/// Lo que este equipo sabe de la clave de cifrado de un amigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Confianza {
    pub verificado: bool,
    /// Su clave cambió desde la última vez que la vimos (se avisa hasta que se mira).
    pub cambio: bool,
}

/// Apunta la clave de cifrado que trae `sync` para un amigo. Si es distinta de la que había,
/// queda marcada como cambiada y deja de estar verificada.
pub fn anotar_clave(db: &Db, amigo: &str, clave: &str) -> AppResult<Confianza> {
    let c = db.conn()?;
    let antes: Option<(String, i64, i64)> = c
        .query_row(
            "SELECT clave_cifrado, verificado, cambio FROM amigos_claves WHERE amigo = ?1",
            [amigo],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    match antes {
        None => {
            c.execute(
                "INSERT INTO amigos_claves (amigo, clave_cifrado) VALUES (?1, ?2)",
                (amigo, clave),
            )?;
            Ok(Confianza::default())
        }
        Some((vieja, verificado, cambio)) if vieja == clave => Ok(Confianza {
            verificado: verificado != 0,
            cambio: cambio != 0,
        }),
        Some(_) => {
            c.execute(
                "UPDATE amigos_claves SET clave_cifrado = ?2, verificado = 0, cambio = 1
                 WHERE amigo = ?1",
                (amigo, clave),
            )?;
            Ok(Confianza {
                verificado: false,
                cambio: true,
            })
        }
    }
}

/// Se comprobó el número de seguridad (o se vio el aviso de cambio).
pub fn verificar(db: &Db, amigo: &str, verificado: bool) -> AppResult<()> {
    db.conn()?.execute(
        "UPDATE amigos_claves SET verificado = ?2, cambio = 0 WHERE amigo = ?1",
        (amigo, i64::from(verificado)),
    )?;
    Ok(())
}

/// Borra la conversación y lo sabido de un amigo (al quitarlo o bloquearlo).
pub fn olvidar(db: &Db, amigo: &str) -> AppResult<()> {
    let c = db.conn()?;
    c.execute("DELETE FROM amigos_mensajes WHERE amigo = ?1", [amigo])?;
    c.execute("DELETE FROM amigos_claves WHERE amigo = ?1", [amigo])?;
    Ok(())
}

/// Borra todo lo de amigos de este equipo (al salir del servicio y pedir que se borre).
pub fn borrar_todo(db: &Db) -> AppResult<()> {
    let c = db.conn()?;
    c.execute("DELETE FROM amigos_mensajes", [])?;
    c.execute("DELETE FROM amigos_claves", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texto(t: &str, enviado: i64) -> Carta {
        Carta {
            contenido: Contenido::Texto { texto: t.into() },
            enviado,
        }
    }

    #[test]
    fn la_conversacion_va_en_orden_y_sin_duplicados() {
        let db = Db::in_memory().unwrap();
        assert!(guardar_recibido(&db, "ana", &texto("hola", 1), "s1", 100).unwrap());
        // El `ack` se perdió y vuelve a llegar: no se duplica.
        assert!(!guardar_recibido(&db, "ana", &texto("hola", 1), "s1", 150).unwrap());
        guardar_mio(&db, "ana", &texto("qué tal", 2), "c1", 200).unwrap();
        let inv = Carta {
            contenido: Contenido::Invitacion {
                codigo: "PB1-x".into(),
            },
            enviado: 3,
        };
        guardar_recibido(&db, "ana", &inv, "s2", 300).unwrap();
        guardar_recibido(&db, "bea", &texto("otra", 1), "s3", 120).unwrap();
        let c = conversacion(&db, "ana", None, 50).unwrap();
        let textos: Vec<_> = c
            .iter()
            .map(|m| (m.texto.as_str(), m.mio, m.tipo.as_str()))
            .collect();
        assert_eq!(
            textos,
            [
                ("hola", false, "texto"),
                ("qué tal", true, "texto"),
                ("PB1-x", false, "invitacion")
            ]
        );
        assert_eq!(c[1].estado, "pendiente");
        // Paginar hacia atrás.
        let antes = conversacion(&db, "ana", Some(c[2].id), 1).unwrap();
        assert_eq!(antes.len(), 1);
        assert_eq!(antes[0].texto, "qué tal");
    }

    #[test]
    fn estados_de_lo_mio_hasta_entregado_y_no_vuelve_atras() {
        let db = Db::in_memory().unwrap();
        let id = guardar_mio(&db, "ana", &texto("x", 1), "c1", 1).unwrap();
        assert_eq!(pendientes(&db).unwrap().len(), 1);
        enviado(&db, "c1", "s9").unwrap();
        assert!(pendientes(&db).unwrap().is_empty());
        poner_estado(&db, "c1", "entregado").unwrap();
        // Un `enviado` tardío (reintento) no la devuelve atrás.
        enviado(&db, "c1", "s9").unwrap();
        poner_estado(&db, "c1", "fallido").unwrap();
        assert_eq!(mio(&db, id).unwrap().unwrap().0.estado, "entregado");
    }

    #[test]
    fn no_leidos_y_leer() {
        let db = Db::in_memory().unwrap();
        anotar_clave(&db, "ana", "k").unwrap();
        guardar_recibido(&db, "ana", &texto("a", 1), "s1", 10).unwrap();
        guardar_recibido(&db, "ana", &texto("b", 1), "s2", 20).unwrap();
        guardar_mio(&db, "ana", &texto("c", 1), "c1", 30).unwrap();
        assert_eq!(no_leidos(&db).unwrap().get("ana"), Some(&2));
        marcar_leido(&db, "ana").unwrap();
        assert!(no_leidos(&db).unwrap().is_empty());
        guardar_recibido(&db, "ana", &texto("d", 1), "s3", 40).unwrap();
        assert_eq!(no_leidos(&db).unwrap().get("ana"), Some(&1));
    }

    #[test]
    fn si_la_clave_de_un_amigo_cambia_deja_de_estar_verificado_y_se_avisa() {
        let db = Db::in_memory().unwrap();
        assert_eq!(
            anotar_clave(&db, "ana", "k1").unwrap(),
            Confianza::default()
        );
        verificar(&db, "ana", true).unwrap();
        assert!(anotar_clave(&db, "ana", "k1").unwrap().verificado);
        let c = anotar_clave(&db, "ana", "k2").unwrap();
        assert!(!c.verificado && c.cambio);
        // Sigue avisando hasta que se mira.
        assert!(anotar_clave(&db, "ana", "k2").unwrap().cambio);
        verificar(&db, "ana", false).unwrap();
        assert!(!anotar_clave(&db, "ana", "k2").unwrap().cambio);
    }

    #[test]
    fn olvidar_y_borrar_todo() {
        let db = Db::in_memory().unwrap();
        anotar_clave(&db, "ana", "k").unwrap();
        guardar_recibido(&db, "ana", &texto("a", 1), "s1", 10).unwrap();
        guardar_recibido(&db, "bea", &texto("b", 1), "s2", 10).unwrap();
        olvidar(&db, "ana").unwrap();
        assert!(conversacion(&db, "ana", None, 10).unwrap().is_empty());
        assert_eq!(conversacion(&db, "bea", None, 10).unwrap().len(), 1);
        borrar_todo(&db).unwrap();
        assert!(conversacion(&db, "bea", None, 10).unwrap().is_empty());
    }
}
