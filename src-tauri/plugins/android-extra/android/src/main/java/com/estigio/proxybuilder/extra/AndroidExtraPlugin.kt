package com.estigio.proxybuilder.extra

import android.app.Activity
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.OpenableColumns
import android.provider.Settings
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.view.WindowManager
import androidx.core.content.FileProvider
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class CompartirArgs {
    lateinit var ruta: String
    var tipo: String = "*/*"
}

@InvokeArg
class DatosArgs {
    lateinit var datos: String
}

@InvokeArg
class TextoArgs {
    lateinit var texto: String
}

@InvokeArg
class SiArgs {
    var si: Boolean = false
}

@InvokeArg
class RutaArgs {
    lateinit var ruta: String
}

@InvokeArg
class UriArgs {
    lateinit var uri: String
}

@InvokeArg
class AvisosArgs {
    /** La carpeta de datos de la app; sin ella, se cancelan. */
    var datos: String? = null
}

@InvokeArg
class AvisoArgs {
    lateinit var amigo: String
    lateinit var titulo: String
    lateinit var cuerpo: String
}

@InvokeArg
class EscuchaArgs {
    lateinit var handler: Channel
}

/**
 * El cofre de la identidad: AES-GCM con una clave del Android Keystore que nunca sale de él. Lo
 * usan el plugin (`cofreSellar`, `cofreAbrir`) y los avisos con la app cerrada (`AvisosAmigos`).
 * 🛑 `ALIAS_COFRE` no se cambia nunca: con otro alias la identidad sellada no se abriría.
 */
internal object Cofre {
    private const val ALMACEN = "AndroidKeyStore"
    const val ALIAS_COFRE = "proxy-builder-identidad"
    private const val CIFRADO = "AES/GCM/NoPadding"
    private const val LARGO_IV = 12

    /** La clave AES del Android Keystore: se crea la primera vez y nunca sale de él. */
    private fun clave(): SecretKey {
        val almacen = KeyStore.getInstance(ALMACEN).apply { load(null) }
        (almacen.getKey(ALIAS_COFRE, null) as? SecretKey)?.let { return it }
        val generador = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ALMACEN)
        generador.init(
            KeyGenParameterSpec.Builder(
                ALIAS_COFRE,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generador.generateKey()
    }

    /** Devuelve el IV (12 bytes) seguido del cifrado. */
    fun sellar(claro: ByteArray): ByteArray {
        val cifrador = Cipher.getInstance(CIFRADO)
        cifrador.init(Cipher.ENCRYPT_MODE, clave())
        return cifrador.iv + cifrador.doFinal(claro)
    }

    fun abrir(sellado: ByteArray): ByteArray {
        val cifrador = Cipher.getInstance(CIFRADO)
        cifrador.init(Cipher.DECRYPT_MODE, clave(), GCMParameterSpec(128, sellado, 0, LARGO_IV))
        return cifrador.doFinal(sellado, LARGO_IV, sellado.size - LARGO_IV)
    }
}

/**
 * Lo que solo hace Android. 🛑 Solo lo llama Rust (`src/android.rs` de la app);
 * la vista web no tiene permisos sobre este plugin. Los ficheros que se
 * comparten o instalan están en la caché de la app, que el `FileProvider` de la
 * plantilla (`res/xml/file_paths.xml`, `<cache-path>`) ya expone.
 */
@TauriPlugin
class AndroidExtraPlugin(private val activity: Activity) : Plugin(activity) {
    private var multicastLock: WifiManager.MulticastLock? = null
    private var canal: Channel? = null

    /** El último fichero que otra app ha abierto con esta y nadie ha recogido. */
    private var pendiente: String? = null

    private fun autoridad() = "${activity.packageName}.fileprovider"

    private fun uriDe(ruta: String): Uri =
        FileProvider.getUriForFile(activity, autoridad(), File(ruta))

    // ── Compartir ───────────────────────────────────────────────────────

    @Command
    fun compartir(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(CompartirArgs::class.java)
            val uri = uriDe(args.ruta)
            val envio = Intent(Intent.ACTION_SEND).apply {
                type = args.tipo
                putExtra(Intent.EXTRA_STREAM, uri)
                clipData = ClipData.newRawUri(File(args.ruta).name, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            val selector = Intent.createChooser(envio, null).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(selector)
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo compartir")
        }
    }

    @Command
    fun compartirTexto(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(TextoArgs::class.java)
            val envio = Intent(Intent.ACTION_SEND).apply {
                type = "text/plain"
                putExtra(Intent.EXTRA_TEXT, args.texto)
            }
            val selector = Intent.createChooser(envio, null).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(selector)
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo compartir")
        }
    }

    // ── El cofre ────────────────────────────────────────────────────────

    private fun resolverDatos(invoke: Invoke, bytes: ByteArray) {
        invoke.resolve(JSObject().apply { put("datos", Base64.encodeToString(bytes, Base64.NO_WRAP)) })
    }

    /** Devuelve el IV (12 bytes) seguido del cifrado. */
    @Command
    fun cofreSellar(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(DatosArgs::class.java)
            resolverDatos(invoke, Cofre.sellar(Base64.decode(args.datos, Base64.NO_WRAP)))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo sellar")
        }
    }

    @Command
    fun cofreAbrir(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(DatosArgs::class.java)
            resolverDatos(invoke, Cofre.abrir(Base64.decode(args.datos, Base64.NO_WRAP)))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo abrir")
        }
    }

    // ── La ventana ──────────────────────────────────────────────────────

    @Command
    fun pantallaEncendida(invoke: Invoke) {
        val args = invoke.parseArgs(SiArgs::class.java)
        activity.runOnUiThread {
            val ventana = activity.window
            if (args.si) {
                ventana.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            } else {
                ventana.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            }
            invoke.resolve()
        }
    }

    @Command
    fun pantallaCompleta(invoke: Invoke) {
        val args = invoke.parseArgs(SiArgs::class.java)
        activity.runOnUiThread {
            try {
                val ventana = activity.window
                val control = WindowCompat.getInsetsController(ventana, ventana.decorView)
                if (args.si) {
                    // Un gesto desde el borde las enseña un momento; «atrás» o ese gesto
                    // las devuelven del todo.
                    control.systemBarsBehavior =
                        WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                    control.hide(WindowInsetsCompat.Type.systemBars())
                } else {
                    control.show(WindowInsetsCompat.Type.systemBars())
                }
                invoke.resolve()
            } catch (ex: Exception) {
                invoke.reject(ex.message ?: "no se pudo cambiar la pantalla completa")
            }
        }
    }

    // ── Wi-Fi ───────────────────────────────────────────────────────────

    @Command
    fun multicast(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SiArgs::class.java)
            synchronized(this) {
                if (args.si) {
                    if (multicastLock == null) {
                        val wifi = activity.applicationContext
                            .getSystemService(Context.WIFI_SERVICE) as WifiManager
                        // Sin contador aquí: quién lo usa (buscar, recibir) lo cuenta
                        // Rust (`android::multicast`) y solo pide soltarlo con el último.
                        multicastLock = wifi.createMulticastLock("proxy-builder-multicast").apply {
                            setReferenceCounted(false)
                        }
                    }
                    if (multicastLock?.isHeld != true) multicastLock?.acquire()
                } else if (multicastLock?.isHeld == true) {
                    multicastLock?.release()
                }
            }
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo cambiar el MulticastLock")
        }
    }

    override fun onDestroy() {
        synchronized(this) {
            if (multicastLock?.isHeld == true) multicastLock?.release()
        }
    }

    // ── Instalar un APK ─────────────────────────────────────────────────

    @Command
    fun instalarApk(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(RutaArgs::class.java)
            val salida = JSObject()
            if (!activity.packageManager.canRequestPackageInstalls()) {
                // Android pide el permiso en un ajuste, no en un diálogo: se abre el de
                // esta app y la persona vuelve y repite.
                val ajuste = Intent(
                    Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:${activity.packageName}"),
                ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                activity.startActivity(ajuste)
                salida.put("lanzado", false)
                invoke.resolve(salida)
                return
            }
            val uri = uriDe(args.ruta)
            val instalar = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(instalar)
            salida.put("lanzado", true)
            invoke.resolve(salida)
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo abrir el instalador")
        }
    }

    // ── Nombre de un content:// ─────────────────────────────────────────

    @Command
    fun nombreDeUri(invoke: Invoke) {
        val salida = JSObject()
        try {
            val args = invoke.parseArgs(UriArgs::class.java)
            val uri = Uri.parse(args.uri)
            activity.contentResolver.query(
                uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null,
            )?.use { c ->
                if (c.moveToFirst()) {
                    val i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (i >= 0) salida.put("nombre", c.getString(i))
                }
            }
        } catch (ex: Exception) {
            // Sin nombre: Rust se las apaña con lo que haya en la URI.
        }
        invoke.resolve(salida)
    }

    /** El nombre que la persona le puso al dispositivo o, si no, su marca y modelo. */
    @Command
    fun nombreEquipo(invoke: Invoke) {
        val salida = JSObject()
        val propio = try {
            Settings.Global.getString(activity.contentResolver, "device_name")
        } catch (ex: Exception) {
            null
        }
        val marca = Build.MANUFACTURER.replaceFirstChar { it.uppercase() }
        val modelo = if (Build.MODEL.startsWith(marca, ignoreCase = true)) {
            Build.MODEL
        } else {
            "$marca ${Build.MODEL}"
        }
        salida.put("nombre", propio?.takeIf { it.isNotBlank() } ?: modelo)
        invoke.resolve(salida)
    }

    // ── Avisos de mensajes con la app cerrada ───────────────────────────

    @Command
    fun avisosAmigos(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(AvisosArgs::class.java)
            AvisosAmigos.programar(activity.applicationContext, args.datos)
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudieron programar los avisos")
        }
    }

    /** Un aviso de amigos con la app abierta (pero sin el foco): el mismo que el del `Worker`. */
    @Command
    fun avisoAmigo(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(AvisoArgs::class.java)
            AvisosAmigos.avisar(activity.applicationContext, args.amigo, args.titulo, args.cuerpo)
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "no se pudo avisar")
        }
    }

    // ── Abrir un fichero desde otra app ─────────────────────────────────

    private fun recoger(intent: Intent?) {
        if (intent == null || intent.action != Intent.ACTION_VIEW) return
        val datos = intent.data ?: return
        // Solo ficheros; los enlaces `proxybuilder://` son del plugin de deep links.
        if (datos.scheme != "content") return
        val uri = datos.toString()
        val canal = this.canal
        if (canal == null) {
            pendiente = uri
        } else {
            canal.send(JSObject().put("uri", uri))
        }
    }

    @Command
    fun escucharAbiertos(invoke: Invoke) {
        val args = invoke.parseArgs(EscuchaArgs::class.java)
        canal = args.handler
        val guardado = pendiente
        pendiente = null
        if (guardado != null) canal?.send(JSObject().put("uri", guardado))
        invoke.resolve()
    }

    override fun load(webView: android.webkit.WebView) {
        super.load(webView)
        // Al volver desde «Recientes» tras morir el proceso, Android entrega otra
        // vez el intent con el que se abrió: ese `.pbdeck` ya se importó.
        val intent = activity.intent
        if (intent != null &&
            (intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY) == 0
        ) {
            recoger(intent)
        }
    }

    override fun onNewIntent(intent: Intent) {
        recoger(intent)
    }
}
