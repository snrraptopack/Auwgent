# Auwgent Compiler Optimization Opportunities

This document outlines potential optimizations for the Auwgent compiler pipeline to improve compilation speed and resource usage.

## Priority 1: High Impact, Moderate Effort

### 1. Parallel File Processing
**Current State**: Files are processed sequentially in loops.

**Opportunity**: Process multiple `.agent` files in parallel using `rayon` or thread pools.
- Each file goes through: parse → check → lower → codegen independently
- Only the final write needs synchronization
- Easy win for multi-file projects

**Impact**: 2-4x speedup for projects with 10+ files

---

### 2. Share IR Between Targets
**Current State**: When generating for multiple targets (ts + python), the full pipeline runs twice per file.

**Opportunity**: Share the IR and only run codegen twice.
- Validation/checking is currently duplicated unnecessarily
- Parse → Check → Lower once, then Codegen for each target

**Impact**: ~50% reduction in compilation time for multi-target projects

---

### 3. Import Graph Caching
**Current State**: Import resolution is recursive and can re-parse the same file multiple times.

**Opportunity**: Build a dependency graph upfront.
- Parse each file exactly once
- Topologically sort and process in order
- Cache parsed imports across multiple root files

**Impact**: Significant speedup for projects with shared dependencies

---

## Priority 2: Medium Impact, Lower Effort

### 4. Incremental Compilation
**Current State**: The compiler re-parses and re-checks everything on each run.

**Opportunity**: Cache intermediate results based on file content.
- Cache parsed ASTs based on file content hash
- Cache type-checked models when imports haven't changed
- Only re-lower IR when AST changes
- Track file modification times and skip unchanged files

**Impact**: 10-100x speedup for watch mode with few changes

---

### 5. Lazy IR Lowering
**Current State**: IR is lowered even if only type checking is needed (LSP).

**Opportunity**: Split validation into stages.
- Parse → Check → Lower as separate phases
- LSP can stop after checking
- CLI can continue to lowering only when needed

**Impact**: Faster LSP diagnostics, reduced memory usage

---

### 6. Watch Mode Batching
**Current State**: Debounce is 80ms, processes files individually.

**Opportunity**: Batch multiple file changes into a single compilation pass.
- Detect which files actually changed vs just touched
- Skip regeneration if output would be identical (content hash)
- Process all changed files in one batch

**Impact**: Smoother watch mode experience, fewer redundant compilations

---

## Priority 3: Lower Impact, Higher Effort

### 7. Memory Allocation Optimization
**Current State**: Several areas create many intermediate allocations.

**Opportunities**:
- `HashMap` clones in type checking (e.g., `bindings.clone()` in workflow.rs)
- String formatting in diagnostics could use `Cow<str>`
- AST cloning during import merging could use `Rc<Element>` or `Arc<Element>`

**Impact**: Reduced memory usage, slightly faster compilation

---

### 8. String Interning
**Current State**: Type names, variable names, and identifiers are cloned frequently.

**Opportunity**: Use string interning (`string-interner` crate).
- Reduce memory footprint
- Speed up string comparisons (pointer equality)
- Particularly beneficial for large projects with many repeated identifiers

**Impact**: 10-20% memory reduction, faster type checking

---

### 9. Diagnostic Batching
**Current State**: Diagnostics are rendered after each file.

**Opportunity**: Batch all diagnostics and render once at the end.
- Group by file for better readability
- Add a `--quiet` mode that only shows errors
- Reduce terminal output overhead

**Impact**: Cleaner output, slightly faster for many files

---

### 10. Config Loading Optimization
**Current State**: `Config::load()` is called multiple times, walks for `auwgent.yml` files repeatedly.

**Opportunity**: Cache config per directory.
- Walk once and cache all discovered configs
- Avoid redundant file system operations

**Impact**: Minor speedup for projects with nested configs

---

## Implementation Roadmap

### Phase 1: Quick Wins (1-2 weeks)
1. Share IR between targets
2. Parallel file processing
3. Watch mode batching improvements

### Phase 2: Caching Infrastructure (2-4 weeks)
1. Import graph caching
2. Incremental compilation with content hashing
3. Lazy IR lowering

### Phase 3: Memory Optimization (1-2 weeks)
1. String interning
2. Reduce HashMap clones
3. Use Rc/Arc for AST sharing

### Phase 4: Polish (1 week)
1. Diagnostic batching
2. Config caching
3. Performance profiling and tuning

---

## Measurement Strategy

Before implementing optimizations:
1. Create benchmark suite with representative projects (small, medium, large)
2. Profile with `cargo flamegraph` to identify actual bottlenecks
3. Measure baseline compilation times
4. Set target improvements (e.g., 2x faster for 100-file projects)

After each optimization:
1. Re-run benchmarks
2. Verify correctness with existing test suite
3. Document actual speedup achieved
