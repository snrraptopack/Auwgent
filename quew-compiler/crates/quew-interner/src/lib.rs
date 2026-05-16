//! # quew-interner
//!
//! Thread-safe string interning for the quew compiler.
//!
//! ## Why `ThreadedRodeo`?
//!
//! The quew compiler is currently single-threaded during parsing and type-checking.
//! However, the architecture explicitly targets parallel multi-file compilation as an
//! adoption milestone. Switching from `Rodeo` (single-threaded, `&mut self` for intern)
//! to `ThreadedRodeo` (multi-threaded, `&self` for both intern and resolve) later would
//! require changing every function signature in `quew-lexer`, `quew-parser`, `quew-ast`,
//! and `quew-checker` that touches interned strings — a breaking change across the entire
//! compiler.
//!
//! By choosing `ThreadedRodeo` now, we pay a minor overhead (DashMap instead of HashMap)
//! but keep all signatures stable. Reads — which dominate during checking — have no
//! Mutex contention.
//!
//! ## Usage
//!
//! ```rust
//! use quew_interner::{Interner, InternedStr};
//! use std::sync::Arc;
//!
//! // Typically created once and passed as Arc<Interner>.
//! let interner = Arc::new(Interner::new());
//!
//! let a: InternedStr = interner.intern("hello");
//! let b: InternedStr = interner.intern("hello");
//! assert_eq!(a, b); // same string → same key
//!
//! let resolved: &str = interner.resolve(a);
//! assert_eq!(resolved, "hello");
//! ```
//!
//! ## Key properties of `InternedStr`
//!
//! - `Copy`, `Clone`, `Eq`, `Hash`, `Ord` — safe to store in sets, use as map keys.
//! - Sized as a `u32` — zero heap allocation, cheap to pass everywhere.
//! - **Cannot** be resolved without the originating `Interner`. Do not store an
//!   `InternedStr` without keeping the `Arc<Interner>` alive.

use lasso::{Spur, ThreadedRodeo};

/// The global string interner type. Pass as `Arc<Interner>` across the compiler.
///
/// Intern a string with [`Interner::intern`] and resolve it back to `&str` with
/// [`Interner::resolve`]. Both operations are `O(1)` on average.
pub struct Interner {
    inner: ThreadedRodeo,
}

impl Interner {
    /// Create a new, empty interner.
    pub fn new() -> Self {
        Self {
            inner: ThreadedRodeo::new(),
        }
    }

    /// Intern a string, returning an [`InternedStr`] handle.
    ///
    /// If the string has been interned before, the same handle is returned.
    /// This is `&self` (not `&mut self`) — safe to call from multiple threads.
    #[inline]
    pub fn intern(&self, s: &str) -> InternedStr {
        InternedStr(self.inner.get_or_intern(s))
    }

    /// Intern a static string, returning an [`InternedStr`] handle.
    ///
    /// Prefer this over [`intern`][Self::intern] when the string is a compile-time
    /// constant — it avoids a hash lookup in some implementations.
    #[inline]
    pub fn intern_static(&self, s: &'static str) -> InternedStr {
        InternedStr(self.inner.get_or_intern_static(s))
    }

    /// Resolve an [`InternedStr`] back to a `&str`.
    ///
    /// The returned `&str` has the same lifetime as `&self`.
    /// Panics in debug mode if the key was not interned from this interner.
    #[inline]
    pub fn resolve(&self, key: InternedStr) -> &str {
        self.inner.resolve(&key.0)
    }

    /// Returns the number of unique strings currently interned.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` when nothing has been interned yet.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

// Debug prints the number of interned strings, not the contents.
// The contents can be enormous; printing them would be useless.
impl std::fmt::Debug for Interner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interner")
            .field("len", &self.len())
            .finish()
    }
}

/// A `u32`-sized handle to an interned string.
///
/// Resolve it back to `&str` via [`Interner::resolve`].
/// Never store a bare `InternedStr` without keeping the originating `Interner` alive.
///
/// # Size
///
/// `size_of::<InternedStr>() == 4` — always. Same cost as storing a raw `u32`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InternedStr(Spur);

impl std::fmt::Debug for InternedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // We don't have the interner here, so print the raw key.
        // For debug-printing with strings, use `interner.resolve(key)` directly.
        write!(f, "InternedStr({:?})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn intern_and_resolve_basic() {
        let interner = Interner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        let c = interner.intern("world");

        assert_eq!(a, b, "same string → same key");
        assert_ne!(a, c, "different strings → different keys");
        assert_eq!(interner.resolve(a), "hello");
        assert_eq!(interner.resolve(c), "world");
    }

    #[test]
    fn intern_static_is_consistent_with_intern() {
        let interner = Interner::new();
        let dynamic = interner.intern("static_str");
        let static_ = interner.intern_static("static_str");
        assert_eq!(dynamic, static_);
    }

    #[test]
    fn interner_len_tracks_unique_strings() {
        let interner = Interner::new();
        assert!(interner.is_empty());
        interner.intern("a");
        interner.intern("b");
        interner.intern("a"); // duplicate — should not increment
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn interned_str_is_copy_and_sized_u32() {
        // Verify the size guarantee: InternedStr must be 4 bytes.
        assert_eq!(std::mem::size_of::<InternedStr>(), 4);
        // Verify it is Copy (compile-time check via move semantics).
        let interner = Interner::new();
        let k = interner.intern("foo");
        let _k2 = k; // copy
        let _resolved = interner.resolve(k); // k still usable
    }

    #[test]
    fn interned_str_ord_allows_sorted_collections() {
        let interner = Interner::new();
        let mut keys: Vec<InternedStr> = vec![
            interner.intern("banana"),
            interner.intern("apple"),
            interner.intern("cherry"),
        ];
        keys.sort(); // must compile; Ord is required
        // We can't assert a human-readable order since lasso assigns keys by
        // insertion order, but we verify sort doesn't panic.
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn thread_safety_concurrent_interns() {
        use std::thread;

        let interner = Arc::new(Interner::new());
        let mut handles = Vec::new();

        // Spawn 8 threads each interning 100 strings.
        for thread_id in 0..8u32 {
            let int = Arc::clone(&interner);
            handles.push(thread::spawn(move || {
                for i in 0..100u32 {
                    let s = format!("t{thread_id}_s{i}");
                    let key = int.intern(&s);
                    // Immediately resolve — no data race.
                    assert_eq!(int.resolve(key), s.as_str());
                }
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }

        // All 8 * 100 = 800 unique strings should be present.
        assert_eq!(interner.len(), 800);
    }

    #[test]
    fn interned_str_usable_as_hashmap_key() {
        use std::collections::HashMap;
        let interner = Interner::new();
        let mut map: HashMap<InternedStr, u32> = HashMap::new();

        let k = interner.intern("key");
        map.insert(k, 42);
        assert_eq!(map[&k], 42);
    }
}
