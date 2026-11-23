import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const algorithm = 'sha256-tree-v1';
const root = 'src-tauri/resources/skills/sideglance';
const git = (...args) => execFileSync('git', args, { maxBuffer: 16 * 1024 * 1024 });

// Paths use UTF-8 and forward slashes on every platform. File contents are exact
// bytes; lengths keep file contents from being confused with the next entry.
export function releaseSkillHash(ref) {
  const hash = createHash('sha256').update(`${algorithm}\0`);
  function visit(tree, prefix = '') {
    const entries = git('ls-tree', '-z', tree)
      .toString('utf8')
      .split('\0')
      .filter(Boolean)
      .map((entry) => {
        const separator = entry.indexOf('\t');
        const metadata = entry.slice(0, separator);
        const name = entry.slice(separator + 1);
        const [mode, type, object] = metadata.split(' ');
        return { mode, type, object, name };
      })
      .sort((a, b) => Buffer.compare(Buffer.from(a.name), Buffer.from(b.name)));
    for (const { mode, type, object, name } of entries) {
      const path = `${prefix}${name}`;
      if (type === 'tree') {
        hash.update(`directory\0${path}\0`);
        visit(object, `${path}/`);
      } else if (type === 'blob' && mode !== '120000') {
        const bytes = git('cat-file', 'blob', object);
        hash.update(`file\0${path}\0${bytes.length}\0`).update(bytes);
      } else {
        throw new Error(`Unsupported skill entry: ${path}`);
      }
    }
  }
  visit(`${ref}:${root}`);
  return hash.digest('hex');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.stdout.write(`${releaseSkillHash(process.argv[2] || 'HEAD')}\n`);
}
