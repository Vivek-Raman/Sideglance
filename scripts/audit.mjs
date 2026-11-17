import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

// Temporary exception approved 2026-11-17. No patched picomatch release exists.
// Vite, tinyglobby and svelte-check use it for build-time filename patterns.
// Remove when upstream publishes a fix. Never exempt production dependencies
// or a different advisory, including one inherited through these same tools.
const exception = 'https://github.com/advisories/GHSA-fc5q-39xc-vjpf';

export function blockingFindings(report, lock) {
  if (report.auditReportVersion !== 2 || !report.vulnerabilities || report.error) {
    throw new Error('Invalid npm audit report');
  }
  const vulnerabilities = report.vulnerabilities;
  function exempt(name, seen = new Set()) {
    const item = vulnerabilities[name];
    if (!item || seen.has(name) || !item.via?.length || !item.nodes?.length) return false;
    if (!item.nodes.every((node) => lock.packages?.[node]?.dev === true)) return false;
    const next = new Set([...seen, name]);
    return item.via.every((source) =>
      typeof source === 'string'
        ? exempt(source, next)
        : source.name === 'picomatch' && source.url === exception,
    );
  }
  return Object.entries(vulnerabilities)
    .filter(([name, item]) => ['high', 'critical'].includes(item.severity) && !exempt(name))
    .map(([name]) => name);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    // Use npm's JS entry point so this also works without a shell on Windows.
    if (!process.env.npm_execpath) throw new Error('Run with npm run audit');
    const result = spawnSync(process.execPath, [process.env.npm_execpath, 'audit', '--json'], {
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024,
    });
    if (result.error || ![0, 1].includes(result.status)) {
      throw result.error ?? new Error(result.stderr || 'npm audit failed');
    }
    const report = JSON.parse(result.stdout);
    console.log(result.stdout);
    const blocked = blockingFindings(report, JSON.parse(readFileSync('package-lock.json', 'utf8')));
    if (blocked.length) throw new Error(`Blocking audit findings: ${blocked.join(', ')}`);
    console.log(`Audit passed with build-only exception for ${exception}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
