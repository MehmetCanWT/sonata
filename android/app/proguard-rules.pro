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
-dontwarn org.slf4j.**
-dontwarn javax.security.**
-dontwarn java.lang.management.**

# SMB2/SMB3 native client (smbj + mbassy + BouncyCastle)
-keepattributes *Annotation*
-keep class com.hierynomus.** { *; }
-keep class net.engio.mbassy.** { *; }
-keep class org.bouncycastle.crypto.** { *; }
-dontwarn com.hierynomus.**
-dontwarn net.engio.mbassy.**
-dontwarn javax.el.**
