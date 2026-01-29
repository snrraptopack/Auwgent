import { URI } from 'langium';
import * as path from 'node:path';

/**
 * Service for resolving import paths to absolute URIs
 */
export interface UriResolver {
    /**
     * Resolves a relative import path to an absolute URI
     * @param importPath - The path from the import statement (may include quotes)
     * @param importingFileUri - The URI of the file containing the import
     * @returns The resolved absolute URI, or undefined if resolution fails
     */
    resolveImportUri(importPath: string, importingFileUri: URI): URI | undefined;
}

/**
 * Default implementation of UriResolver for Auwgent
 */
export class AuwgentUriResolver implements UriResolver {
    
    resolveImportUri(importPath: string, importingFileUri: URI): URI | undefined {
        try {
            // Remove quotes from import path
            const cleanPath = importPath.replace(/['"]/g, '');
            
            // Add .agent extension if not present
            const pathWithExtension = cleanPath.endsWith('.agent') 
                ? cleanPath 
                : `${cleanPath}.agent`;
            
            // Resolve relative to importing file's directory
            const importingPath = importingFileUri.fsPath;
            const importingDir = path.dirname(importingPath);
            const resolvedPath = path.resolve(importingDir, pathWithExtension);
            
            return URI.file(resolvedPath);
        } catch (error) {
            // Log resolution failure for debugging
            console.error(`Failed to resolve import path "${importPath}" from ${importingFileUri.toString()}:`, error);
            return undefined;
        }
    }
}
