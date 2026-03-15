# Roadmap: JVM Support & Shared FFI Architecture

This document tracks the long-term plan for cross-language interoperability and the introduction of Java/JVM support to the Auwgent ecosystem.

## 1. JVM (Java/Kotlin/Android) Support

To provide a "freefly" experience for JVM developers, we will prioritize a native integration that doesn't impose heavy requirements like GraalVM on the end-user.

### Strategy: JNI (Java Native Interface)
*   **Approach**: Use the `jni` crate in Rust to create standard native libraries (`.dll`, `.so`, `.dylib`).
*   **Distribution**: Package the native binaries inside a standard JAR file.
*   **Advantage**: Works out-of-the-box with any standard Java/Kotlin setup (Spring Boot, Android, Desktop apps) without requiring GraalVM's Polyglot runtime.
*   **Alternative (GraalVM)**: Keep GraalVM Polyglot as an advanced option for environments already using it, but don't make it the baseline.

## 2. Shared FFI Bridge (`ir-ffi-bridge`)

Currently, TypeScript and Python bindings duplicate about 80% of their "glue" logic (marshaling JSON, managing the Tokio runtime, wrapping `AuwgentEngine`). We will consolidate this into a shared Rust crate.

### The Unified Bridge Pattern
Instead of each language target implementing its own engine wrapper, we will create a centralized `ir-ffi-bridge` crate:

*   **Shared Core Logic**:
    *   `AuwgetEngine` instance management (Arc/Mutex).
    *   Global or per-instance Tokio Runtime management.
    *   JSON-string-based API surface (The "Message Bridge").
    *   Common driver registration and context management.

*   **Thin Language Wrappers**:
    *   **TypeScript (`auwgent-napi`)**: Only handles N-API conversions and calls the Shared Bridge.
    *   **Python (`auwgent-pyo3`)**: Only handles PyO3 conversions and calls the Shared Bridge.
    *   **Java (`auwgent-jni`)**: Only handles JNI boilerplate and calls the Shared Bridge.

### Benefits
1.  **70% Reduction in Maintenance**: Bugs in the FFI layer only need to be fixed once.
2.  **Capability Parity**: All languages get new features (like streaming references) the moment they are added to the Bridge.
3.  **Performance**: Highly optimized async management and cross-thread communication handled once in high-quality Rust.

---
*Created on 2026-03-15 to guide the structural evolution of Auwgent Interop.*
