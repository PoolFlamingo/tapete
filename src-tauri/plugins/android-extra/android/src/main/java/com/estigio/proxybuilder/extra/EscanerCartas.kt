package com.estigio.proxybuilder.extra

import android.app.Activity
import android.graphics.Color
import android.graphics.drawable.Drawable
import android.util.Size
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleOwner
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * **La cámara de «Identificar por foto»**: la vista previa a pantalla completa DETRÁS de la vista
 * web (que se vuelve transparente: la interfaz del escáner es la de React) y los fotogramas para
 * Rust por JNI (`Nativo.fotograma`), solo cuando Rust los quiere (`Nativo.quiere`) y como mucho
 * uno cada `INTERVALO_MS`. Lo pesado (encontrar la carta, leerla) es de Rust (`reconocer/vivo.rs`).
 *
 * Como el modo `windowed` del escáner de QR (`tauri-plugin-barcode-scanner`), con la misma versión
 * de CameraX. CameraX va atada al ciclo de vida de la actividad: en segundo plano se para sola.
 */
class EscanerCartas(private val activity: Activity, private val webView: WebView) {
    private var vista: PreviewView? = null
    private var proveedor: ProcessCameraProvider? = null
    private var camara: Camera? = null
    private var fondo: Drawable? = null
    private var vistaAbierta = false
    private var hilo: ExecutorService? = null
    private var ultimo = 0L
    private var bufer: ByteArray? = null

    val abierto: Boolean
        get() = vista != null

    /** Abre la cámara; `alFallar` recibe el motivo si no se pudo. Desde el hilo de la interfaz. */
    fun abrir(alAbrir: () -> Unit, alFallar: (String) -> Unit) {
        if (vista != null) {
            alAbrir()
            return
        }
        val previa = PreviewView(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            )
            scaleType = PreviewView.ScaleType.FILL_CENTER
        }
        val padre = webView.parent as? ViewGroup
        if (padre == null) {
            alFallar("sin-vista")
            return
        }
        padre.addView(previa, 0)
        webView.bringToFront()
        fondo = webView.background
        vistaAbierta = true
        webView.setBackgroundColor(Color.TRANSPARENT)
        vista = previa
        hilo = Executors.newSingleThreadExecutor()

        val futuro = ProcessCameraProvider.getInstance(activity)
        futuro.addListener({
            try {
                val p = futuro.get()
                proveedor = p
                atar(p, previa)
                alAbrir()
            } catch (ex: Exception) {
                cerrar()
                alFallar(ex.message ?: "camara")
            }
        }, ContextCompat.getMainExecutor(activity))
    }

    private fun atar(p: ProcessCameraProvider, previa: PreviewView) {
        val selector = CameraSelector.DEFAULT_BACK_CAMERA
        if (!p.hasCamera(selector)) throw IllegalStateException("sin-camara")
        val vistaPrevia = Preview.Builder().build().also {
            it.setSurfaceProvider(previa.surfaceProvider)
        }
        val resolucion = ResolutionSelector.Builder()
            .setResolutionStrategy(
                ResolutionStrategy(
                    Size(1280, 720),
                    ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER,
                ),
            )
            .build()
        val analisis = ImageAnalysis.Builder()
            .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
            .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_RGBA_8888)
            .setResolutionSelector(resolucion)
            .build()
        val ejecutor = hilo ?: return
        analisis.setAnalyzer(ejecutor) { imagen -> analizar(imagen) }
        p.unbindAll()
        camara = p.bindToLifecycle(activity as LifecycleOwner, selector, vistaPrevia, analisis)
    }

    /** Copia el fotograma (quitando el relleno de cada fila) y se lo da a Rust si lo quiere. */
    private fun analizar(imagen: ImageProxy) {
        try {
            val ahora = System.currentTimeMillis()
            if (ahora - ultimo < INTERVALO_MS || !Nativo.quiere()) return
            val plano = imagen.planes[0]
            val ancho = imagen.width
            val alto = imagen.height
            val fila = ancho * 4
            val tam = fila * alto
            val datos = bufer?.takeIf { it.size == tam } ?: ByteArray(tam).also { bufer = it }
            val origen = plano.buffer
            origen.rewind()
            if (plano.rowStride == fila) {
                origen.get(datos, 0, tam)
            } else {
                for (y in 0 until alto) {
                    origen.position(y * plano.rowStride)
                    origen.get(datos, y * fila, fila)
                }
            }
            if (Nativo.fotograma(datos, ancho, alto, imagen.imageInfo.rotationDegrees)) ultimo = ahora
        } catch (_: Throwable) {
            // Un fotograma que falla se tira: vendrá otro.
        } finally {
            imagen.close()
        }
    }

    /** Enciende o apaga la linterna; `false` si no hay flash. */
    fun linterna(si: Boolean): Boolean {
        val c = camara ?: return false
        if (!c.cameraInfo.hasFlashUnit()) return false
        c.cameraControl.enableTorch(si)
        return si
    }

    /** Cierra la cámara y devuelve su fondo a la vista web. Desde el hilo de la interfaz. */
    fun cerrar() {
        proveedor?.unbindAll()
        proveedor = null
        camara = null
        vista?.let { (it.parent as? ViewGroup)?.removeView(it) }
        vista = null
        // El fondo de antes, tal cual (también si no tenía).
        if (vistaAbierta) webView.background = fondo
        vistaAbierta = false
        fondo = null
        hilo?.shutdown()
        hilo = null
        bufer = null
    }

    companion object {
        /** Lo menos que pasa entre dos fotogramas para Rust (≈ 8 por segundo como mucho). */
        const val INTERVALO_MS = 120L
    }
}
