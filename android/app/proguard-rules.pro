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

# WebView JS bridge
-keep class dev.sonata.android.SonataViewModel$WebViewJsCipherSolver { *; }
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}

# Third-party warnings suppressed for R8
-dontwarn androidx.profileinstaller.**
-dontwarn androidx.media3.**
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
