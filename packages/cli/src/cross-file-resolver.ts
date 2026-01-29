import { Model, isHelper, isTypeDeclaration, isNamedPrompt, Helper, TypeDeclaration, NamedPrompt } from "auwgent-language";
import * as fs from 'node:fs';
import * as path from 'node:path';

/**
 * Resolves cross-file references and collects all dependencies
 */
export class CrossFileResolver {
    private processedFiles = new Set<string>();
    private helpers = new Map<string, Helper>();
    private types = new Map<string, TypeDeclaration>();
    private prompts = new Map<string, NamedPrompt>();

    /**
     * Resolve all imports starting from the main file
     * @param mainModel - The parsed main model
     * @param mainFilePath - The absolute path to the main file
     * @param parseFile - Function to parse a file and return its Model
     */
    async resolveImports(
        mainModel: Model,
        mainFilePath: string,
        parseFile: (filePath: string) => Promise<Model | null>
    ): Promise<{
        helpers: Map<string, Helper>;
        types: Map<string, TypeDeclaration>;
        prompts: Map<string, NamedPrompt>;
    }> {
        // Process the main file and all its imports recursively
        await this.processModel(mainModel, mainFilePath, parseFile);

        return {
            helpers: this.helpers,
            types: this.types,
            prompts: this.prompts
        };
    }

    private async processModel(model: Model, currentFilePath: string, parseFile: (filePath: string) => Promise<Model | null>): Promise<void> {
        // Mark this file as processed
        this.processedFiles.add(currentFilePath);

        // Collect exported elements from this file
        for (const element of model.elements) {
            if (isHelper(element) && element.exported) {
                this.helpers.set(element.name, element);
            } else if (isTypeDeclaration(element) && element.exported) {
                this.types.set(element.name, element);
            } else if (isNamedPrompt(element) && element.exported) {
                this.prompts.set(element.name, element);
            }
        }

        // Process imports
        for (const importStmt of model.imports) {
            const importPath = importStmt.importPath.replace(/['"]/g, '');
            const resolvedPath = this.resolveImportPath(importPath, currentFilePath);

            if (!resolvedPath) {
                console.warn(`Warning: Could not resolve import path "${importPath}" from ${currentFilePath}`);
                continue;
            }

            // Skip if already processed
            if (this.processedFiles.has(resolvedPath)) {
                continue;
            }

            // Parse and process the imported file
            const importedModel = await parseFile(resolvedPath);
            if (importedModel) {
                await this.processModel(importedModel, resolvedPath, parseFile);
            } else {
                console.warn(`Warning: Could not parse imported file: ${resolvedPath}`);
            }
        }
    }

    private resolveImportPath(importPath: string, currentFilePath: string): string | null {
        try {
            // Add .agent extension if not present
            const pathWithExtension = importPath.endsWith('.agent') 
                ? importPath 
                : `${importPath}.agent`;

            // Resolve relative to current file's directory
            const currentDir = path.dirname(currentFilePath);
            const resolvedPath = path.resolve(currentDir, pathWithExtension);

            // Check if file exists
            if (fs.existsSync(resolvedPath)) {
                return resolvedPath;
            }

            return null;
        } catch (error) {
            console.error(`Error resolving import path "${importPath}":`, error);
            return null;
        }
    }

    /**
     * Clear all cached data
     */
    clear(): void {
        this.processedFiles.clear();
        this.helpers.clear();
        this.types.clear();
        this.prompts.clear();
    }
}
