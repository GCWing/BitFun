# Kotlin serialization serializers are referenced from generated code.
-keepattributes *Annotation*,InnerClasses,EnclosingMethod

# SQLDelight schemas and Ktor engines are referenced through generated/service metadata.
-keep class com.bitfun.mobile.core.persistence.db.** { *; }
-dontwarn org.slf4j.**
