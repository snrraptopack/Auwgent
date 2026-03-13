const os = require('os');
const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

const VERSION = require('./package.json').version;
const TAG = `v${VERSION}`; 
const REPO = 'snrraptopack/Auwgent';

const platform = os.platform();
const arch = os.arch();

const BIN_DIR = path.join(__dirname, 'bin');
const ASSET_DIR = path.join(__dirname, 'assets');
const IS_CI = process.env.CI === 'true';
const IS_GLOBAL_INSTALL = process.env.npm_config_global === 'true';
const DOWNLOAD_BASE = process.env.AUWGENT_CLI_DOWNLOAD_BASE || `https://github.com/${REPO}/releases/download`;

let target = '';
let archiveExt = '.tar.gz';
let binaryExt = '';

if (platform === 'win32' && arch === 'x64') {
    target = 'x86_64-pc-windows-msvc';
    archiveExt = '.zip';
    binaryExt = '.exe';
} else if (platform === 'darwin' && arch === 'x64') {
    target = 'x86_64-apple-darwin';
} else if (platform === 'darwin' && arch === 'arm64') {
    target = 'aarch64-apple-darwin';
} else if (platform === 'linux' && arch === 'x64') {
    target = 'x86_64-unknown-linux-gnu';
} else {
    console.error(`Unsupported platform or architecture: ${platform} ${arch}`);
    process.exit(1);
}

function downloadFile(url, dest) {
    return new Promise((resolve, reject) => {
        const file = fs.createWriteStream(dest);
        const request = https.get(url, (res) => {
            if (res.statusCode === 301 || res.statusCode === 302) {
                file.close(() => {
                    fs.unlink(dest, () => {
                        downloadFile(res.headers.location, dest).then(resolve).catch(reject);
                    });
                });
                return;
            }
            if (res.statusCode !== 200) {
                file.close(() => {
                    fs.unlink(dest, () => {
                        reject(new Error(`Failed to download ${url}, status code: ${res.statusCode}`));
                    });
                });
                return;
            }
            res.pipe(file);
            file.on('finish', () => {
                file.close(() => resolve());
            });
        });

        request.setTimeout(20000, () => {
            request.destroy(new Error(`Download timed out for ${url}`));
        });

        request.on('error', (err) => {
            fs.unlink(dest, () => reject(err));
        });
    });
}

function extractFile(file, ext) {
    const cwd = BIN_DIR;
    console.log(`Extracting ${file}...`);
    try {
        if (ext === '.zip') {
            execSync(`powershell -Command "Expand-Archive -Path '${file}' -DestinationPath '${cwd}' -Force"`, { stdio: 'inherit' });
        } else {
            execSync(`tar -xzf "${file}" -C "${cwd}"`, { stdio: 'inherit' });
        }
        fs.unlinkSync(file);
    } catch (e) {
        console.error("Failed to extract file", e);
        process.exit(1);
    }
}

function walkDir(dir, maxDepth = 5, depth = 0) {
    if (depth > maxDepth || !fs.existsSync(dir)) {
        return [];
    }
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    const files = [];
    for (const entry of entries) {
        const entryPath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            files.push(...walkDir(entryPath, maxDepth, depth + 1));
        } else {
            files.push(entryPath);
        }
    }
    return files;
}

function normalizeExtractedLayout(expectedBins) {
    const allFiles = walkDir(BIN_DIR);
    for (const expectedBin of expectedBins) {
        const directPath = path.join(BIN_DIR, expectedBin);
        if (fs.existsSync(directPath)) {
            continue;
        }
        const nested = allFiles.find((p) => path.basename(p) === expectedBin);
        if (nested) {
            fs.copyFileSync(nested, directPath);
        }
    }
}

async function downloadWithRetry(url, dest, attempts = 3) {
    let lastError;
    for (let i = 1; i <= attempts; i++) {
        try {
            if (fs.existsSync(dest)) {
                fs.unlinkSync(dest);
            }
            await downloadFile(url, dest);
            return;
        } catch (err) {
            lastError = err;
            console.warn(`Download attempt ${i}/${attempts} failed: ${err.message}`);
        }
    }
    throw lastError;
}

function assertBinariesPresent(expectedBins) {
    const missing = expectedBins.filter((binName) => !fs.existsSync(path.join(BIN_DIR, binName)));
    if (missing.length > 0) {
        const files = walkDir(BIN_DIR).map((p) => path.relative(BIN_DIR, p));
        throw new Error(
            [
                `Install completed but required binaries are missing: ${missing.join(', ')}`,
                `Expected location: ${BIN_DIR}`,
                `Found files: ${files.length > 0 ? files.join(', ') : '(none)'}`,
                `Bundled archive checked: ${path.join(ASSET_DIR, `auwgent-${target}${archiveExt}`)}`,
                `Download URL used: ${DOWNLOAD_BASE}/${TAG}/auwgent-${target}${archiveExt}`
            ].join('\n')
        );
    }
}

async function main() {
    if (!fs.existsSync(BIN_DIR)) {
        fs.mkdirSync(BIN_DIR, { recursive: true });
    }

    const filename = `auwgent-${target}${archiveExt}`;
    const url = `${DOWNLOAD_BASE}/${TAG}/${filename}`;
    const dest = path.join(BIN_DIR, filename);
    const bundledArchive = path.join(ASSET_DIR, filename);
    const expectedBins = [`auwgent${binaryExt}`, `auwgent-lsp${binaryExt}`];

    if (fs.existsSync(bundledArchive)) {
        console.log(`Using bundled Auwgent archive: ${bundledArchive}`);
        fs.copyFileSync(bundledArchive, dest);
    } else {
        console.log(`Downloading Auwgent CLI from ${url}...`);
        await downloadWithRetry(url, dest);
    }

    extractFile(dest, archiveExt);
    normalizeExtractedLayout(expectedBins);

    // On unix, ensure executable permissions for all binaries
    if (platform !== 'win32') {
        for (const bin of expectedBins) {
            const extractedBin = path.join(BIN_DIR, bin);
            if (fs.existsSync(extractedBin)) {
                fs.chmodSync(extractedBin, 0o755);
            }
        }
    }
    assertBinariesPresent(expectedBins);
    console.log("Auwgent CLI installed successfully.");
}

main().catch(e => {
    // Fail hard in CI and global installs; allow local repo installs to proceed.
    if (IS_GLOBAL_INSTALL || IS_CI) {
        console.error(e.message || e);
        process.exit(1);
    } else {
        console.warn("Failed to download binary during local dev install. Ignoring.");
        console.warn(e.message || e);
    }
});
