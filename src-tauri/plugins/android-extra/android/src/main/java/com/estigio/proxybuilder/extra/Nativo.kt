package com.estigio.proxybuilder.extra

/**
 * La parte de Rust de la app (`libproxy_builder_lib.so`). 🛑 Los nombres de la clase y de cada
 * método los implementa Rust por JNI (`Java_com_estigio_proxybuilder_extra_Nativo_<método>`):
 * no se cambian (`consumer-rules.pro` los conserva en la release).
 */
object Nativo {
    /** Los avisos que hay que enseñar, en JSON (`[{ amigo, titulo, cuerpo }]`), o `null` (`amigos/fondo.rs`). */
    @JvmStatic
    external fun comprobar(datos: String, clave: ByteArray): String?

    /** ¿Quiere Rust un fotograma del escáner ahora? (`reconocer/vivo.rs`) */
    @JvmStatic
    external fun quiere(): Boolean

    /**
     * Un fotograma del escáner: RGBA seguido (sin relleno entre filas) y los grados que hay que
     * girarlo, en el sentido del reloj, para que quede derecho. `true` si Rust lo cogió.
     */
    @JvmStatic
    external fun fotograma(rgba: ByteArray, ancho: Int, alto: Int, giro: Int): Boolean
}
