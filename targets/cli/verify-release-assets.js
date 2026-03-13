const https = require('https');
const pkg = require('./package.json');

const version = pkg.version;
const expectedTag = `v${version}`;
const refTag = process.env.GITHUB_REF_NAME;
const owner = process.env.AUWGENT_REPO_OWNER || 'snrraptopack';
const repo = process.env.AUWGENT_REPO_NAME || 'Auwgent';
const githubToken = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;

if (refTag && refTag !== expectedTag) {
  console.error(`Tag/version mismatch: git tag is ${refTag}, package version expects ${expectedTag}`);
  process.exit(1);
}

const assets = [
  'auwgent-x86_64-unknown-linux-gnu.tar.gz',
  'auwgent-x86_64-pc-windows-msvc.zip',
  'auwgent-x86_64-apple-darwin.tar.gz',
  'auwgent-aarch64-apple-darwin.tar.gz'
];

function getJson(url) {
  return new Promise((resolve, reject) => {
    const headers = {
      'User-Agent': 'auwgent-cli-release-check'
    };

    if (githubToken) {
      headers.Authorization = `Bearer ${githubToken}`;
    }

    const req = https.request(url, { method: 'GET', headers }, (res) => {
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => {
        const status = res.statusCode || 0;
        const body = Buffer.concat(chunks).toString('utf8');
        if (status < 200 || status >= 300) {
          reject(new Error(`HTTP ${status} for ${url}: ${body}`));
          return;
        }
        try {
          resolve(JSON.parse(body));
        } catch (err) {
          reject(new Error(`Invalid JSON response from ${url}: ${err.message}`));
        }
      });
    });

    req.setTimeout(15000, () => {
      req.destroy(new Error(`Timeout while checking ${url}`));
    });

    req.on('error', reject);
    req.end();
  });
}

async function main() {
  const url = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${expectedTag}`;
  const release = await getJson(url);
  const assetNames = new Set((release.assets || []).map((asset) => asset.name));

  console.log(`Found release ${expectedTag} with ${(release.assets || []).length} assets.`);

  for (const name of assets) {
    process.stdout.write(`Checking ${name} ... `);
    if (!assetNames.has(name)) {
      throw new Error(`Missing release asset: ${name}`);
    }
    console.log('OK');
  }

  console.log(`All required release assets exist for ${expectedTag}.`);
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
