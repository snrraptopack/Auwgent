import * as vscode from 'vscode';
import * as path from 'node:path';
import * as fs from 'node:fs';
import {
	LanguageClient,
	RevealOutputChannelOn,
	type LanguageClientOptions,
	type ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
	outputChannel = vscode.window.createOutputChannel('Auwgent Rust LSP');
	context.subscriptions.push(outputChannel);

	context.subscriptions.push(
		vscode.workspace.onDidChangeConfiguration(event => {
			if (event.affectsConfiguration('auwgent.serverPath')) {
				void restartLanguageClient(context, false);
			}
		}),
		vscode.commands.registerCommand('auwgent.restartLanguageServer', async () => {
			await restartLanguageClient(context, true);
		}),
		vscode.commands.registerCommand('auwgent.showLanguageServerOutput', () => {
			outputChannel?.show(true);
		})
	);

	await restartLanguageClient(context, false);
	context.subscriptions.push({
		dispose: () => {
			void client?.stop();
		}
	});
}

export async function deactivate(): Promise<void> {
	if (client) {
		await client.stop();
		client = undefined;
	}
}

async function restartLanguageClient(context: vscode.ExtensionContext, showSuccessMessage: boolean): Promise<void> {
	if (client) {
		await client.stop();
		client = undefined;
	}

	try {
		client = await startLanguageClient(context);
		if (showSuccessMessage) {
			void vscode.window.showInformationMessage('Auwgent Rust LSP restarted.');
		}
	} catch (error) {
		await handleStartupError(context, error);
	}
}

async function startLanguageClient(context: vscode.ExtensionContext): Promise<LanguageClient> {
	const { serverOptions, resolvedPath } = await resolveServerOptions(context);
	outputChannel?.appendLine(`Starting language server: ${resolvedPath}`);

	const clientOptions: LanguageClientOptions = {
		documentSelector: [
			{ scheme: 'file', language: 'auwgent' },
			{ scheme: 'untitled', language: 'auwgent' },
		],
		diagnosticCollectionName: 'auwgent',
		outputChannel,
		revealOutputChannelOn: RevealOutputChannelOn.Never,
	};

	const languageClient = new LanguageClient(
		'auwgent-rust',
		'Auwgent Rust LSP',
		serverOptions,
		clientOptions
	);

	await languageClient.start();
	context.subscriptions.push(languageClient);
	return languageClient;
}

async function resolveServerOptions(context: vscode.ExtensionContext): Promise<{ serverOptions: ServerOptions; resolvedPath: string }> {
	const explicitPath = vscode.workspace.getConfiguration('auwgent').get<string>('serverPath');
	const configuredPath = explicitPath && explicitPath.trim().length > 0 ? explicitPath : undefined;
	const bundledCandidates = [
		configuredPath,
		...findBundledServerCandidates(context),
	].filter((candidate): candidate is string => Boolean(candidate));

	for (const candidate of bundledCandidates) {
		if (fs.existsSync(candidate)) {
			return {
				serverOptions: { command: candidate },
				resolvedPath: candidate,
			};
		}
	}

	const searchedLocations = bundledCandidates.map(candidate => ` - ${candidate}`).join('\n');
	throw new Error(
		[
			'Auwgent Rust LSP binary not found.',
			'Searched:',
			searchedLocations || ' - no candidate paths were produced',
		].join('\n')
	);
	
}

function findBundledServerCandidates(context: vscode.ExtensionContext): string[] {
	const compilerRoots = new Set<string>();
	const extensionRoot = context.extensionUri.fsPath;
	compilerRoots.add(path.resolve(extensionRoot, '..', 'auwgent-compiler'));

	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		compilerRoots.add(path.join(folder.uri.fsPath, 'auwgent-compiler'));

		if (path.basename(folder.uri.fsPath).toLowerCase() === 'auwgent-compiler') {
			compilerRoots.add(folder.uri.fsPath);
		}

		if (path.basename(folder.uri.fsPath).toLowerCase() === 'extension-vscode') {
			compilerRoots.add(path.resolve(folder.uri.fsPath, '..', 'auwgent-compiler'));
		}
	}

	return [...compilerRoots].flatMap(compilerRoot => [
		path.join(compilerRoot, 'target', 'debug', executableName('auwgent-lsp')),
		path.join(compilerRoot, 'target', 'release', executableName('auwgent-lsp')),
	]);
}

async function handleStartupError(context: vscode.ExtensionContext, error: unknown): Promise<void> {
	const message = error instanceof Error ? error.message : String(error);
	outputChannel?.appendLine(message);
	outputChannel?.show(true);

	const buildTask = await vscode.tasks.fetchTasks({ type: 'shell' });
	const rustBuild = buildTask.find(task => task.name === 'Build Auwgent Rust LSP');
	const actions = rustBuild ? ['Build Rust LSP', 'Open Output'] : ['Open Output'];
	const selection = await vscode.window.showErrorMessage(
		'Auwgent Rust LSP could not be started.',
		...actions
	);

	if (selection === 'Build Rust LSP' && rustBuild) {
		outputChannel?.appendLine('Building Rust LSP and retrying startup...');
		const exitCode = await executeTaskAndWait(rustBuild);

		if (exitCode === 0 || exitCode === undefined) {
			await restartLanguageClient(context, true);
		} else {
			void vscode.window.showErrorMessage(`Build Auwgent Rust LSP failed with exit code ${exitCode}.`);
		}
		return;
	}

	if (selection === 'Open Output') {
		outputChannel?.show(true);
	}

	if (rustBuild) {
		outputChannel?.appendLine(
			'Run the workspace task "Build Auwgent Rust LSP", then use the "Auwgent: Restart Language Server" command.'
		);
	} else {
		outputChannel?.appendLine(
			'Build auwgent-compiler/crates/auwgent-lsp before using the extension.'
		);
	}
}

async function executeTaskAndWait(task: vscode.Task): Promise<number | undefined> {
	const execution = await vscode.tasks.executeTask(task);

	return await new Promise<number | undefined>(resolve => {
		const disposables: vscode.Disposable[] = [];
		const finish = (exitCode: number | undefined) => {
			while (disposables.length > 0) {
				disposables.pop()?.dispose();
			}
			resolve(exitCode);
		};

		disposables.push(
			vscode.tasks.onDidEndTaskProcess(event => {
				if (event.execution === execution) {
					finish(event.exitCode);
				}
			}),
			vscode.tasks.onDidEndTask(event => {
				if (event.execution === execution) {
					finish(undefined);
				}
			})
		);
	});
}

function executableName(base: string): string {
	return process.platform === 'win32' ? `${base}.exe` : base;
}
