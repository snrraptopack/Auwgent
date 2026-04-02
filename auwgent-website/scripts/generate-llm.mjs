import fs from 'node:fs';
import path from 'node:path';

const rootDir = process.cwd();
const astroConfigPath = path.join(rootDir, 'astro.config.mjs');
const docsRoot = path.join(rootDir, 'src', 'content', 'docs');

function readUtf8(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

function extractSidebarArray(source) {
  const keyIndex = source.indexOf('sidebar');
  if (keyIndex === -1) {
    throw new Error('Could not find sidebar in astro.config.mjs');
  }

  const bracketStart = source.indexOf('[', keyIndex);
  if (bracketStart === -1) {
    throw new Error('Could not find sidebar array start');
  }

  let i = bracketStart;
  let depth = 0;
  let inSingle = false;
  let inDouble = false;
  let inTemplate = false;
  let inLineComment = false;
  let inBlockComment = false;
  let escaped = false;

  for (; i < source.length; i += 1) {
    const ch = source[i];
    const next = source[i + 1];

    if (inLineComment) {
      if (ch === '\n') inLineComment = false;
      continue;
    }

    if (inBlockComment) {
      if (ch === '*' && next === '/') {
        inBlockComment = false;
        i += 1;
      }
      continue;
    }

    if (inSingle) {
      if (!escaped && ch === "'") inSingle = false;
      escaped = !escaped && ch === '\\';
      continue;
    }

    if (inDouble) {
      if (!escaped && ch === '"') inDouble = false;
      escaped = !escaped && ch === '\\';
      continue;
    }

    if (inTemplate) {
      if (!escaped && ch === '`') inTemplate = false;
      escaped = !escaped && ch === '\\';
      continue;
    }

    if (ch === '/' && next === '/') {
      inLineComment = true;
      i += 1;
      continue;
    }

    if (ch === '/' && next === '*') {
      inBlockComment = true;
      i += 1;
      continue;
    }

    if (ch === "'") {
      inSingle = true;
      escaped = false;
      continue;
    }

    if (ch === '"') {
      inDouble = true;
      escaped = false;
      continue;
    }

    if (ch === '`') {
      inTemplate = true;
      escaped = false;
      continue;
    }

    if (ch === '[') {
      depth += 1;
    } else if (ch === ']') {
      depth -= 1;
      if (depth === 0) {
        return source.slice(bracketStart, i + 1);
      }
    }
  }

  throw new Error('Could not find sidebar array end');
}

function parseSidebar(source) {
  const sidebarArrayText = extractSidebarArray(source);
  const parseFn = new Function(`return (${sidebarArrayText});`);
  return parseFn();
}

function collectSlugs(sidebarItems, out = []) {
  for (const item of sidebarItems) {
    if (!item || typeof item !== 'object') continue;

    if (typeof item.slug === 'string') {
      out.push(item.slug);
    }

    if (Array.isArray(item.items)) {
      collectSlugs(item.items, out);
    }
  }
  return out;
}

function resolveDocFile(slug) {
  const mdxPath = path.join(docsRoot, `${slug}.mdx`);
  const mdPath = path.join(docsRoot, `${slug}.md`);

  if (fs.existsSync(mdxPath)) return mdxPath;
  if (fs.existsSync(mdPath)) return mdPath;

  throw new Error(`Sidebar slug not found in docs content: ${slug}`);
}

function fileHeader(filePath) {
  const relative = path.relative(rootDir, filePath).replace(/\//g, '\\');
  return `===== FILE: .\\${relative} =====`;
}

function buildBundle(docPaths) {
  const sections = docPaths.map((docPath) => {
    const body = readUtf8(docPath).trimEnd();
    return `${fileHeader(docPath)}\n${body}`;
  });
  return `${sections.join('\n\n')}\n`;
}

function writeOutputs(bundle) {
  const outputs = [
    path.join(rootDir, 'llm.txt'),
    path.join(rootDir, 'public', 'llm.txt'),
    path.join(rootDir, 'public', 'llms.txt'),
  ];

  for (const outputPath of outputs) {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, bundle, 'utf8');
  }

  return outputs;
}

function main() {
  const astroConfig = readUtf8(astroConfigPath);
  const sidebar = parseSidebar(astroConfig);
  const slugs = [...new Set(collectSlugs(sidebar))];
  const docPaths = slugs.map(resolveDocFile);
  const bundle = buildBundle(docPaths);
  const outputs = writeOutputs(bundle);

  console.log(`Generated LLM bundle from ${docPaths.length} docs.`);
  for (const outputPath of outputs) {
    console.log(`Wrote: ${path.relative(rootDir, outputPath)}`);
  }
}

main();
