package com.estigio.proxybuilder.extra

import android.Manifest
import android.app.ActivityManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import org.json.JSONArray
import java.io.File
import java.util.concurrent.TimeUnit

/** La parte de Rust (`amigos/fondo.rs` de la app, en `libproxy_builder_lib.so`). */
object Nativo {
    /** Los avisos que hay que enseñar, en JSON (`[{ amigo, titulo, cuerpo }]`), o `null`. */
    @JvmStatic
    external fun comprobar(datos: String, clave: ByteArray): String?
}

/**
 * **Avisos de mensajes con la app cerrada** (plan Tapete, D4b). WorkManager lo despierta cada
 * ~15 minutos, solo con red; Rust mira el servidor sin recoger nada (los mensajes se quedan hasta
 * que se abre la app) y dice qué avisar: «Mensaje de <apodo>», nunca el texto.
 *
 * Con la app delante no hace nada: ya avisa ella (`amigos/avisar.rs`). 🛑 Nunca escribe ni mueve
 * los ficheros de la identidad: solo los lee (eso lo hace la app al arrancar).
 */
class AvisosAmigos(contexto: Context, parametros: WorkerParameters) :
    Worker(contexto, parametros) {

    companion object {
        private const val TRABAJO = "tapete-avisos-amigos"
        private const val PREFERENCIAS = "tapete-avisos-amigos"
        private const val DATOS = "datos"

        /** El canal del plugin de notificaciones de Tauri: los avisos de la app abierta y
         * los de aquí comparten los ajustes del sistema. */
        private const val CANAL = "default"

        /** Con la carpeta de datos lo programa (si ya lo estaba, se queda como estaba); sin
         * ella, lo cancela. */
        fun programar(contexto: Context, datos: String?) {
            val trabajos = WorkManager.getInstance(contexto)
            val preferencias = contexto.getSharedPreferences(PREFERENCIAS, Context.MODE_PRIVATE)
            if (datos == null) {
                trabajos.cancelUniqueWork(TRABAJO)
                preferencias.edit().remove(DATOS).apply()
                return
            }
            preferencias.edit().putString(DATOS, datos).apply()
            val peticion = PeriodicWorkRequestBuilder<AvisosAmigos>(15, TimeUnit.MINUTES)
                .setConstraints(
                    Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build(),
                )
                .build()
            trabajos.enqueueUniquePeriodicWork(
                TRABAJO,
                ExistingPeriodicWorkPolicy.KEEP,
                peticion,
            )
        }
    }

    override fun doWork(): Result {
        if (enPrimerPlano() || !puedeAvisar()) return Result.success()
        val datos = applicationContext
            .getSharedPreferences(PREFERENCIAS, Context.MODE_PRIVATE)
            .getString(DATOS, null) ?: return Result.success()
        val clave = leerClave(File(datos, "identidad")) ?: return Result.success()
        val json = try {
            System.loadLibrary("proxy_builder_lib")
            Nativo.comprobar(datos, clave)
        } catch (ex: Throwable) {
            null
        } finally {
            clave.fill(0)
        }
        // Un fallo (sin servidor, sin red de verdad): se vuelve a probar en la siguiente vuelta.
        if (json == null) return Result.success()
        try {
            val avisos = JSONArray(json)
            for (i in 0 until avisos.length()) {
                val a = avisos.getJSONObject(i)
                avisar(a.getString("amigo"), a.getString("titulo"), a.getString("cuerpo"))
            }
        } catch (ex: Exception) {
            // JSON roto: nada que enseñar.
        }
        return Result.success()
    }

    private fun enPrimerPlano(): Boolean {
        val estado = ActivityManager.RunningAppProcessInfo()
        ActivityManager.getMyMemoryState(estado)
        return estado.importance <= ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND
    }

    private fun puedeAvisar(): Boolean {
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(
                applicationContext,
                Manifest.permission.POST_NOTIFICATIONS,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        return NotificationManagerCompat.from(applicationContext).areNotificationsEnabled()
    }

    /**
     * La clave como la elige la app al arrancar (`red::identidad::cargar_o_crear`): la que esté
     * en claro (sin sellar todavía o tras un importar que no pudo sellar) y, si no, la sellada.
     */
    private fun leerClave(carpeta: File): ByteArray? {
        val enClaro = File(carpeta, "clave")
        if (enClaro.isFile) {
            val b = enClaro.readBytes()
            if (b.size == 32) return b
        }
        val sellada = File(carpeta, "clave.sellada")
        if (!sellada.isFile) return null
        return try {
            Cofre.abrir(sellada.readBytes()).takeIf { it.size == 32 }
        } catch (ex: Exception) {
            null
        }
    }

    private fun avisar(amigo: String, titulo: String, cuerpo: String) {
        val contexto = applicationContext
        val gestor = contexto.getSystemService(NotificationManager::class.java)
        if (gestor.getNotificationChannel(CANAL) == null) {
            gestor.createNotificationChannel(
                NotificationChannel(CANAL, titulo, NotificationManager.IMPORTANCE_DEFAULT),
            )
        }
        val abrir = contexto.packageManager.getLaunchIntentForPackage(contexto.packageName)
        val alTocar = abrir?.let {
            PendingIntent.getActivity(
                contexto,
                0,
                it,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        }
        val aviso = NotificationCompat.Builder(contexto, CANAL)
            .setSmallIcon(R.drawable.aviso_tapete)
            .setContentTitle(titulo)
            .setContentText(cuerpo)
            .setAutoCancel(true)
            .setContentIntent(alTocar)
            .build()
        try {
            // Uno por amigo: el siguiente suyo sustituye al anterior.
            NotificationManagerCompat.from(contexto).notify(amigo.hashCode(), aviso)
        } catch (ex: SecurityException) {
            // Sin permiso desde que se comprobó.
        }
    }
}
