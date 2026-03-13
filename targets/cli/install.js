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

let target = '';
let ext = '.tar.gz';

if (platform === 'win32' && arch === 'x64') {
    target = 'x86_64-pc-windows-msvc';
    ext = '.zip';
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
        let file = fs.createWriteStream(dest);
        https.get(url, (res) => {
            if (res.statusCode === 301 || res.statusCode === 302) {
                return downloadFile(res.headers.location, dest).then(resolve).catch(reject);
            }
            if (res.statusCode !== 200) {
                return reject(new Error(`Failed to download ${url}, status code: ${res.statusCode}`));
            }
            res.pipe(file);
            file.on('finish', () => {
                file.close(resolve);
            });
        }).on('error', (err) => {
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

async function main() {
    if (!fs.existsSync(BIN_DIR)) {
        fs.mkdirSync(BIN_DIR, { recursive: true });
    }
    
    for (const bin of ['auwgent', 'auwgent-lsp']) {
        const filename = `${bin}-${target}${ext}`;
        const url = `https://github.com/${REPO}/releases/download/${TAG}/${filename}`;
        const dest = path.join(BIN_DIR, filename);

        console.log(`Downloading ${bin} CLI from ${url}...`);
        await downloadFile(url, dest);
        extractFile(dest, ext);
        
        // On unix, ensure executable permissions
        if (platform !== 'win32') {
            const extractedBin = path.join(BIN_DIR, bin);
            if (fs.existsSync(extractedBin)) {
                fs.chmodSync(extractedBin, 0o755);
            }
        }
    }
    console.log("Auwgent CLI installed successfully.");
}

main().catch(e => {
    // Only fail strictly if it's not a local dev installation
    if (process.env.npm_config_global) {
        console.error(e);
        process.exit(1);
    } else {
        console.warn("Failed to download binary during local dev install. Ignoring.");
    }
});
