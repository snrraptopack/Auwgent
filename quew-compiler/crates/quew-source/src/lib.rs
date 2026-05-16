//! # quew-source
//!
//! **Single responsibility:** tracks which source file each span belongs to and maps
//! byte offsets to human-readable line/column positions.
//!
//! ## Why this crate exists
//!
//! `quew-errors::Span` only holds `(start, end)` byte offsets. In a multi-file compiler,
//! a span alone does not tell you *which* file it is in. This crate adds that layer:
//!
//! - [`SourceId`] — a `u32`-sized, `Copy` handle that identifies one source file.
//! - [`SourceFile`] — owns the raw text of one file and its line-start table.
//! - [`SourceMap`] — the registry; maps `SourceId` → `SourceFile`, and resolves
//!   `(SourceId, byte_offset)` → `(line, column)`.
//!
//! ## Design rules
//!
//! 1. This crate has no knowledge of tokens, AST nodes, or types.
//! 2. All string-like paths use `InternedStr` so file names are interned once.
//! 3. `SourceId` is `Copy`, `Eq`, `Hash` — store it freely in spans and diagnostics.
//! 4. Line/column indices are 1-based (matching what editors and ariadne expect).

use quew_errors::Span;
use quew_interner::{InternedStr, Interner};
use std::sync::{Arc, RwLock};

/// A `u32`-sized, `Copy` handle that uniquely identifies one source file within
/// a [`SourceMap`]. The value `0` is reserved; all valid ids start at `1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(u32);

impl SourceId {
    /// Returns the raw numeric id. Treat as opaque; do not construct manually.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A 1-based line + 1-based column position within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number (byte offset within the line).
    pub col: u32,
}

/// Owns the raw source text of a single file and a precomputed table of
/// byte offsets where each line begins (for fast offset → line/col lookup).
pub struct SourceFile {
    /// Interned display path (e.g., `"src/main.quew"`).
    pub path: InternedStr,
    /// The full source text.
    pub text: Arc<str>,
    /// `line_starts[i]` is the byte offset of the start of line `i + 1`.
    /// Always has at least one entry (`line_starts[0] == 0`).
    line_starts: Vec<u32>,
}

impl SourceFile {
    /// Create a new `SourceFile`, computing the line-start table eagerly.
    pub fn new(path: InternedStr, text: impl Into<Arc<str>>) -> Self {
        let text: Arc<str> = text.into();
        // Build the line start table: byte offset of the first character of each line.
        let mut line_starts = vec![0u32];
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                // Next line starts at the byte after \n.
                line_starts.push((i + 1) as u32);
            }
        }
        Self { path, text, line_starts }
    }

    /// Convert a byte offset into a 1-based `(line, col)` pair.
    ///
    /// If `offset` is beyond the end of the file, clamps to the last position.
    pub fn offset_to_line_col(&self, offset: usize) -> LineCol {
        let offset = offset.min(self.text.len()) as u32;
        // Binary search for the largest line_start ≤ offset.
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,        // offset falls exactly on a line start
            Err(i) => i - 1,   // offset is within this line
        };
        let line_start = self.line_starts[line_index];
        LineCol {
            line: (line_index + 1) as u32,
            col: (offset - line_start + 1),
        }
    }

    /// Convert a [`Span`] into a pair of `(start_line_col, end_line_col)`.
    pub fn span_to_line_col(&self, span: Span) -> (LineCol, LineCol) {
        (self.offset_to_line_col(span.start), self.offset_to_line_col(span.end))
    }

    /// The total number of lines in this file.
    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }
}

/// The central registry of all source files in a compilation session.
///
/// Thread-safe: uses `RwLock` internally so it can be shared via `Arc<SourceMap>`
/// across the parallel file-resolution phase in `quew-resolve`.
pub struct SourceMap {
    interner: Arc<Interner>,
    /// Indexed by `SourceId::raw() - 1` (ids are 1-based).
    files: RwLock<Vec<SourceFile>>,
}

impl SourceMap {
    /// Create an empty `SourceMap`.
    pub fn new(interner: Arc<Interner>) -> Self {
        Self {
            interner,
            files: RwLock::new(Vec::new()),
        }
    }

    /// Register a new source file and return its [`SourceId`].
    ///
    /// The same path can be registered more than once (e.g., for re-compilation);
    /// each call produces a distinct `SourceId`.
    pub fn add(&self, path: &str, text: impl Into<Arc<str>>) -> SourceId {
        let interned_path = self.interner.intern(path);
        let file = SourceFile::new(interned_path, text);
        let mut files = self.files.write().expect("SourceMap write lock poisoned");
        let id = SourceId((files.len() + 1) as u32); // 1-based
        files.push(file);
        id
    }

    /// Look up a `SourceFile` by id.
    ///
    /// Panics if the id was not produced by this `SourceMap`.
    pub fn get(&self, id: SourceId) -> impl std::ops::Deref<Target = SourceFile> + '_ {
        let files = self.files.read().expect("SourceMap read lock poisoned");
        // SAFETY: We hand out a guard that keeps the read lock alive.
        // We use an index-based approach here to avoid lifetime issues.
        let index = (id.raw() - 1) as usize;
        assert!(index < files.len(), "SourceId {:?} not registered in this SourceMap", id);
        // Return the guard + index; the caller dereferences into the file.
        SourceMapGuard { guard: files, index }
    }

    /// Total number of source files registered.
    pub fn file_count(&self) -> usize {
        self.files.read().expect("SourceMap read lock poisoned").len()
    }
}

/// A guard returned by [`SourceMap::get`] that keeps the read lock alive.
struct SourceMapGuard<'a> {
    guard: std::sync::RwLockReadGuard<'a, Vec<SourceFile>>,
    index: usize,
}

impl std::ops::Deref for SourceMapGuard<'_> {
    type Target = SourceFile;
    fn deref(&self) -> &Self::Target {
        &self.guard[self.index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_interner() -> Arc<Interner> {
        Arc::new(Interner::new())
    }

    // ── SourceFile tests ──────────────────────────────────────────────────────

    #[test]
    fn source_file_single_line_offset_to_line_col() {
        let interner = make_interner();
        let path = interner.intern("test.quew");
        let file = SourceFile::new(path, "hello world");
        // Single line: offset 0 → line 1, col 1
        assert_eq!(file.offset_to_line_col(0), LineCol { line: 1, col: 1 });
        // Offset 5 → line 1, col 6
        assert_eq!(file.offset_to_line_col(5), LineCol { line: 1, col: 6 });
    }

    #[test]
    fn source_file_multiline_offset_to_line_col() {
        let interner = make_interner();
        let path = interner.intern("test.quew");
        // "hello\nworld\n" — line 1 starts at 0, line 2 at 6, line 3 at 12
        let file = SourceFile::new(path, "hello\nworld\n");
        assert_eq!(file.offset_to_line_col(0), LineCol { line: 1, col: 1 });
        assert_eq!(file.offset_to_line_col(5), LineCol { line: 1, col: 6 }); // '\n'
        assert_eq!(file.offset_to_line_col(6), LineCol { line: 2, col: 1 }); // 'w'
        assert_eq!(file.offset_to_line_col(11), LineCol { line: 2, col: 6 }); // '\n'
        assert_eq!(file.offset_to_line_col(12), LineCol { line: 3, col: 1 });
    }

    #[test]
    fn source_file_empty_text() {
        let interner = make_interner();
        let path = interner.intern("empty.quew");
        let file = SourceFile::new(path, "");
        // Empty file: still has one "line" at offset 0
        assert_eq!(file.line_count(), 1);
        assert_eq!(file.offset_to_line_col(0), LineCol { line: 1, col: 1 });
    }

    #[test]
    fn source_file_offset_beyond_end_is_clamped() {
        let interner = make_interner();
        let path = interner.intern("test.quew");
        let file = SourceFile::new(path, "hi");
        // Offset 999 beyond the 2-byte file — should not panic, clamps to end
        let lc = file.offset_to_line_col(999);
        assert_eq!(lc.line, 1);
    }

    #[test]
    fn source_file_span_to_line_col() {
        let interner = make_interner();
        let path = interner.intern("test.quew");
        let file = SourceFile::new(path, "hello\nworld");
        let span = Span::new(0, 5);
        let (start, end) = file.span_to_line_col(span);
        assert_eq!(start, LineCol { line: 1, col: 1 });
        assert_eq!(end, LineCol { line: 1, col: 6 });
    }

    #[test]
    fn source_file_line_count() {
        let interner = make_interner();
        let path = interner.intern("test.quew");
        // 3 newlines → 4 lines (line 4 may be empty)
        let file = SourceFile::new(path, "a\nb\nc\n");
        assert_eq!(file.line_count(), 4);
    }

    // ── SourceId tests ────────────────────────────────────────────────────────

    #[test]
    fn source_id_is_copy_and_eq() {
        let id = SourceId(1);
        let id2 = id; // copy
        assert_eq!(id, id2);
        assert_eq!(id.raw(), 1);
    }

    // ── SourceMap tests ───────────────────────────────────────────────────────

    #[test]
    fn source_map_add_and_get() {
        let interner = make_interner();
        let map = SourceMap::new(Arc::clone(&interner));
        let id = map.add("src/main.quew", "agent Foo {}");
        assert_eq!(map.file_count(), 1);
        let file = map.get(id);
        assert_eq!(&*file.text, "agent Foo {}");
    }

    #[test]
    fn source_map_ids_are_unique_per_registration() {
        let interner = make_interner();
        let map = SourceMap::new(Arc::clone(&interner));
        let id1 = map.add("a.quew", "agent A {}");
        let id2 = map.add("b.quew", "agent B {}");
        assert_ne!(id1, id2);
        assert_eq!(map.file_count(), 2);
    }

    #[test]
    fn source_map_same_path_produces_distinct_ids() {
        let interner = make_interner();
        let map = SourceMap::new(Arc::clone(&interner));
        // Registering the same path twice (e.g., re-compilation) gives distinct ids.
        let id1 = map.add("agent.quew", "v1");
        let id2 = map.add("agent.quew", "v2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn source_map_resolves_line_col_via_get() {
        let interner = make_interner();
        let map = SourceMap::new(Arc::clone(&interner));
        let id = map.add("test.quew", "hello\nworld");
        let file = map.get(id);
        let lc = file.offset_to_line_col(6); // 'w' in "world"
        assert_eq!(lc, LineCol { line: 2, col: 1 });
    }

    #[test]
    fn source_map_thread_safety_concurrent_adds() {
        use std::thread;
        let interner = make_interner();
        let map = Arc::new(SourceMap::new(Arc::clone(&interner)));
        let mut handles = Vec::new();
        for i in 0..8u32 {
            let map = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                let path = format!("file_{i}.quew");
                let content = format!("agent Agent{i} {{}}");
                map.add(&path, content.as_str())
            }));
        }
        let ids: Vec<SourceId> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        // All 8 ids must be distinct.
        let unique: std::collections::HashSet<SourceId> = ids.into_iter().collect();
        assert_eq!(unique.len(), 8);
        assert_eq!(map.file_count(), 8);
    }
}
