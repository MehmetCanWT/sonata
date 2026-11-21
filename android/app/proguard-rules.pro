# R8 / ProGuard keep rules for Sonata Android release builds.

# JNA & UniFFI native bridge
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class * extends com.sun.jna.Structure { *; }
-keepclassmembers class * extends com.sun.jna.Structure {
    public <fields>;
    public <methods>;
}
-keep class uniffi.sonata_android.** { *; }
-keepclassmembers class uniffi.sonata_android.** { *; }
-dontwarn com.sun.jna.**
-dontwarn java.awt.**

# App entry points and ViewModels
-keep class dev.sonata.android.MainActivity { *; }
-keep class dev.sonata.android.PlaybackService { *; }
-keep class dev.sonata.android.SonataViewModel { *; }

# Media3 / ExoPlayer
-keep class androidx.media3.** { *; }
-dontwarn androidx.media3.**

# Coil 3 & OkHttp
-keep class coil3.** { *; }
-dontwarn coil3.**
-dontwarn okhttp3.**
-dontwarn okio.**
-dontwarn javax.annotation.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**

# Kotlin coroutines & runtime
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}
-keepclassmembers class kotlinx.coroutines.** {
    volatile <fields>;
}
