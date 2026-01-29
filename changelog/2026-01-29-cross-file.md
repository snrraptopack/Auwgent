# Changelog - January 29, 2026 (Cross-File Referencing)

## Added
- **Cross-file referencing system** for modular agent development
- Import statements with explicit type annotations (`helper`, `type`, `prompt`)
- Export keyword for helpers, types, and prompts
- Named imports with optional aliases: `import { helper DataFetcher as DF } from "./helpers"`
- Wildcard imports with namespaces: `import * as helpers from "./helpers"`
- URI resolver for relative path resolution with automatic `.agent` extension
- Import cache manager for performance optimization
- Scope computation service to collect and publish exported symbols
- Enhanced scope provider to resolve imported symbols across files
- Comprehensive validation system:
  - Import path resolution validation
  - Symbol existence and export status validation
  - Import kind validation (ensures `helper` imports are actually helpers, etc.)
  - Circular dependency detection with full cycle path reporting
  - Export dependency warnings for incomplete public APIs
- Test files demonstrating all import/export features
- Comprehensive test guide for manual testing

## Changed
- Grammar extended with `FileImport`, `NamedImports`, `ImportSpecifier`, `WildcardImport` rules
- `ImportKind` enum added: `'helper' | 'type' | 'prompt'`
- `Model` now includes `imports` array before `elements`
- All exportable elements (`Helper`, `TypeDeclaration`, `NamedPrompt`) have `exported: boolean` property
- Scope provider now handles both local and imported symbols with proper precedence
- Validator enhanced with services injection for cross-file validation

## Technical Details

### Grammar Changes
```langium
FileImport:
    'import' (NamedImports | WildcardImport) 'from' importPath=STRING;

ImportSpecifier:
    kind=ImportKind imported=[Exportable:ID] ('as' alias=ID)?;

ImportKind returns string:
    'helper' | 'type' | 'prompt';
```

### Architecture
- **UriResolver**: Converts relative import paths to absolute URIs
- **ImportCacheManager**: Caches resolved URIs and exported symbols
- **ScopeComputation**: Collects exported symbols for global index
- **ScopeProvider**: Resolves references by querying imported symbols
- **Validator**: Validates imports, exports, and detects circular dependencies

### Example Usage
```auwgent
// helpers.agent
export type User {
    name: string,
    email: string
}

export helper DataFetcher {
    description: "Fetches data"
    input { url: string }
    output { data: string }
}

// main.agent
import { helper DataFetcher, type User } from "./helpers"

agent MainAgent {
    input { user: User }
    helpers { DataFetcher }
}
```

## Files Added
- packages/language/src/auwgent-uri-resolver.ts
- packages/language/src/auwgent-import-cache.ts
- packages/language/src/scope/auwgent-scope-computation.ts
- manual-testing/helpers.agent
- manual-testing/main.agent
- manual-testing/test-imports.agent
- manual-testing/circular-a.agent
- manual-testing/circular-b.agent
- manual-testing/CROSS_FILE_TEST_GUIDE.md

## Files Modified
- packages/language/src/auwgent.langium
- packages/language/src/auwgent-validator.ts
- packages/language/src/auwgent-module.ts
- packages/language/src/scope/auwgent-scope.ts
- packages/language/src/generated/ast.ts (regenerated)

## Next Steps
- Task 6: IDE features (autocomplete, go-to-definition, find references, rename)
- Task 7: Workspace indexing optimization
- Task 8-10: Comprehensive testing (unit, property-based, integration)
- Task 11: Documentation updates

## Breaking Changes
None - existing single-file agents continue to work without modification.

## Notes
- Local definitions take precedence over imported symbols
- Import kind validation ensures type safety across files
- Circular dependencies are detected and reported with full cycle path
- Export dependency warnings help maintain clean public APIs
