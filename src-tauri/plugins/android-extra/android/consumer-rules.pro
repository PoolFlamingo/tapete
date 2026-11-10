# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile# `Nativo.comprobar` lo implementa Rust por JNI (`amigos/fondo.rs`): el nombre de la clase y
# del método no pueden cambiar.
-keep class com.estigio.proxybuilder.extra.Nativo { *; }
-keep class com.estigio.proxybuilder.extra.AvisosAmigos { *; }

# WorkManager guarda su cola en una base de datos de Room y la crea por reflexión
# (`WorkDatabase_Impl()`) al arrancar el proceso (`androidx.startup.InitializationProvider`).
# Room solo pide `-keep class * extends RoomDatabase`, que en el modo completo de R8 (el de AGP 9)
# ya no conserva el constructor: la release 4.0.0 se cerraba nada más abrirse. Hay que conservarlo.
-keep class * extends androidx.room.RoomDatabase { <init>(); }
