import { test } from 'node:test';
import assert from 'node:assert/strict';
import { blockingFindings } from './audit.mjs';

const advisory = { name: 'picomatch', url: 'https://github.com/advisories/GHSA-fc5q-39xc-vjpf' };
function fixture() {
  return {
    report: {
      auditReportVersion: 2,
      vulnerabilities: {
        picomatch: { severity: 'high', via: [advisory], nodes: ['node_modules/picomatch'] },
        vite: { severity: 'high', via: ['picomatch'], nodes: ['node_modules/vite'] },
      },
    },
    lock: {
      packages: { 'node_modules/picomatch': { dev: true }, 'node_modules/vite': { dev: true } },
    },
  };
}
test('allows the specific dev-only advisory and its dependent findings', () => {
  const { report, lock } = fixture();
  assert.deepEqual(blockingFindings(report, lock), []);
});
test('blocks another advisory in the same dependency chain', () => {
  const { report, lock } = fixture();
  report.vulnerabilities.picomatch.via.push({
    ...advisory,
    url: 'https://github.com/advisories/other',
  });
  assert.deepEqual(blockingFindings(report, lock), ['picomatch', 'vite']);
});
test('blocks production and missing lockfile entries', () => {
  for (const value of [{}, undefined]) {
    const { report, lock } = fixture();
    lock.packages['node_modules/picomatch'] = value;
    assert.deepEqual(blockingFindings(report, lock), ['picomatch', 'vite']);
  }
});
test('blocks unrelated critical findings, missing references and cycles', () => {
  for (const via of [[{ name: 'other', url: 'other' }], ['missing'], ['vite']]) {
    const { report, lock } = fixture();
    report.vulnerabilities.vite = { severity: 'critical', via, nodes: ['node_modules/vite'] };
    assert.deepEqual(blockingFindings(report, lock), ['vite']);
  }
});
test('rejects audit errors and unsupported reports', () => {
  for (const report of [{}, { auditReportVersion: 2, vulnerabilities: {}, error: {} }]) {
    assert.throws(() => blockingFindings(report, {}));
  }
});
