import type { AstNode, LangiumCoreServices, LangiumDocument } from 'langium';
import chalk from 'chalk';
import * as path from 'node:path';
import * as fs from 'node:fs';
import { URI } from 'langium';

export async function extractDocument(fileName: string, services: LangiumCoreServices): Promise<LangiumDocument> {
    const extensions = services.LanguageMetaData.fileExtensions;
    if (!extensions.includes(path.extname(fileName))) {
        console.error(chalk.yellow(`Please choose a file with one of these extensions: ${extensions}.`));
        process.exit(1);
    }

    if (!fs.existsSync(fileName)) {
        console.error(chalk.red(`File ${fileName} does not exist.`));
        process.exit(1);
    }

    const document = await services.shared.workspace.LangiumDocuments.getOrCreateDocument(URI.file(path.resolve(fileName)));
    
    // Also load imported documents
    const model = document.parseResult?.value as any;
    const importedDocs: LangiumDocument[] = [];
    
    if (model && model.imports) {
        for (const importStmt of model.imports) {
            const importPath = importStmt.importPath.replace(/['"]/g, '');
            const pathWithExtension = importPath.endsWith('.agent') ? importPath : `${importPath}.agent`;
            const resolvedPath = path.resolve(path.dirname(fileName), pathWithExtension);
            
            if (fs.existsSync(resolvedPath)) {
                const importedDoc = await services.shared.workspace.LangiumDocuments.getOrCreateDocument(URI.file(resolvedPath));
                importedDocs.push(importedDoc);
            }
        }
    }
    
    // Build all documents together
    await services.shared.workspace.DocumentBuilder.build([document, ...importedDocs], { validation: true });

    const validationErrors = (document.diagnostics ?? []).filter(e => e.severity === 1);
    if (validationErrors.length > 0) {
        console.error(chalk.red('There are validation errors:'));
        for (const validationError of validationErrors) {
            console.error(chalk.red(
                `line ${validationError.range.start.line + 1}: ${validationError.message} [${document.textDocument.getText(validationError.range)}]`
            ));
        }
        process.exit(1);
    }

    return document;
}

export async function extractAstNode<T extends AstNode>(fileName: string, services: LangiumCoreServices): Promise<T> {
    return (await extractDocument(fileName, services)).parseResult?.value as T;
}

interface FilePathData {
    destination: string,
    name: string
}

export function extractDestinationAndName(destination: string): FilePathData {
    return {
        destination: path.dirname(destination),
        name: path.basename(destination)
    };
}
