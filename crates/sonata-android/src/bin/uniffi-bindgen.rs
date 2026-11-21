//! UniFFI binding generator CLI for `sonata-android`.
//!
//! 1. **Kotlin Bindings**: Invokes `uniffi::uniffi_bindgen_main()` so Gradle generates bindings
//!    guaranteed to match the compiled `libsonata_android.so` UniFFI version.

fn main() {
    uniffi::uniffi_bindgen_main();
}
