import { execSync } from 'child_process';
import * as path from 'path';
import * as url from 'url';

const __dirname = url.fileURLToPath(new URL('.', import.meta.url));
const rootPath = path.resolve(__dirname);

// If user supplies 'ts' or 'python', use that. Else default to both.
const targetArg = process.argv[2] ?? 'both';
const sourceAgent = path.join(rootPath, 'manual-testing', 'main.agent');

if (targetArg === 'ts' || targetArg === 'both') {
    // Run TS compilation
    console.log(`Generating TS definitions for ${sourceAgent}...`);
    execSync(`node packages/cli/bin/cli.js generate "${sourceAgent}" "${path.join(rootPath, 'ir-runtime', 'typescript', 'verification')}" --target ts`, {
        cwd: rootPath,
        stdio: 'inherit'
    });
}

if (targetArg === 'python' || targetArg === 'both') {
    // Run Python compilation
    console.log(`\nGenerating Python definitions for ${sourceAgent}...`);
    execSync(`node packages/cli/bin/cli.js generate "${sourceAgent}" "${path.join(rootPath, 'ir-runtime', 'python', 'verification')}" --target python`, {
        cwd: rootPath,
        stdio: 'inherit'
    });
}

console.log("\nFinished generating! 🎉");
