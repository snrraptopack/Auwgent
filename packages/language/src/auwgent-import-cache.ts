import { URI } from 'langium';
import { AstNodeDescription } from 'langium';

/**
 * Cache structure for import resolution
 */
export interface ImportCache {
    // Map from (importing file URI + import path) to resolved URI
    resolvedUris: Map<string, URI | undefined>;
    
    // Map from file URI to its exported symbols
    exportedSymbols: Map<string, AstNodeDescription[]>;
    
    // Dependency graph for circular dependency detection
    dependencyGraph: Map<string, Set<string>>;
}

/**
 * Manages caching for import resolution to optimize performance
 */
export class ImportCacheManager {
    private cache: ImportCache = {
        resolvedUris: new Map(),
        exportedSymbols: new Map(),
        dependencyGraph: new Map()
    };
    
    /**
     * Invalidates cache entries for a modified file
     * @param fileUri - The URI of the file that was modified
     */
    invalidate(fileUri: URI): void {
        const uriString = fileUri.toString();
        
        // Clear exported symbols for this file
        this.cache.exportedSymbols.delete(uriString);
        
        // Clear resolved URIs that reference this file
        for (const [key, value] of this.cache.resolvedUris.entries()) {
            if (value?.toString() === uriString) {
                this.cache.resolvedUris.delete(key);
            }
        }
        
        // Update dependency graph
        this.cache.dependencyGraph.delete(uriString);
    }
    
    /**
     * Gets cached resolved URI
     * @param importingUri - The URI of the file containing the import
     * @param importPath - The import path string
     * @returns The cached resolved URI, or undefined if not cached
     */
    getResolvedUri(importingUri: URI, importPath: string): URI | undefined {
        const key = `${importingUri.toString()}::${importPath}`;
        return this.cache.resolvedUris.get(key);
    }
    
    /**
     * Caches resolved URI
     * @param importingUri - The URI of the file containing the import
     * @param importPath - The import path string
     * @param resolvedUri - The resolved absolute URI
     */
    setResolvedUri(importingUri: URI, importPath: string, resolvedUri: URI): void {
        const key = `${importingUri.toString()}::${importPath}`;
        this.cache.resolvedUris.set(key, resolvedUri);
        
        // Update dependency graph
        const deps = this.cache.dependencyGraph.get(importingUri.toString()) || new Set();
        deps.add(resolvedUri.toString());
        this.cache.dependencyGraph.set(importingUri.toString(), deps);
    }
    
    /**
     * Gets cached exported symbols for a file
     * @param fileUri - The URI of the file
     * @returns The cached exported symbols, or undefined if not cached
     */
    getExportedSymbols(fileUri: URI): AstNodeDescription[] | undefined {
        return this.cache.exportedSymbols.get(fileUri.toString());
    }
    
    /**
     * Caches exported symbols for a file
     * @param fileUri - The URI of the file
     * @param symbols - The exported symbols
     */
    setExportedSymbols(fileUri: URI, symbols: AstNodeDescription[]): void {
        this.cache.exportedSymbols.set(fileUri.toString(), symbols);
    }
    
    /**
     * Gets the dependency graph
     * @returns The dependency graph mapping file URIs to their dependencies
     */
    getDependencyGraph(): Map<string, Set<string>> {
        return this.cache.dependencyGraph;
    }
    
    /**
     * Clears all cached data
     */
    clear(): void {
        this.cache.resolvedUris.clear();
        this.cache.exportedSymbols.clear();
        this.cache.dependencyGraph.clear();
    }
}
