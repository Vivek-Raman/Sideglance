import test from 'node:test';
import assert from 'node:assert/strict';
import { countExpiryKeys, pointKey, projectNames, viewerRows } from '../src/viewer/rows.ts';
import type { StatusEntry } from '../src/shared/types.ts';

const entry = (session: string, branch: string, times: (string | null)[]): StatusEntry => ({
  folder_name: 'demo',
  session_id: session,
  branch,
  updated_at: '2026-11-03T03:00:00Z',
  points: times.map((submitted_at, index) => ({
    id: session + index,
    text: session + index,
    done: false,
    link: null,
    submitted_at,
  })),
});
const entries = [
  entry('a', 'main', ['2026-11-03T01:00:00Z', '2026-11-03T04:00:00Z']),
  entry('b', 'main', ['2026-11-03T02:00:00Z', null]),
  entry('c', 'other', ['2026-11-03T05:00:00Z']),
];

test('newest submissions first by default, across sessions within each branch', () => {
  const rows = viewerRows(entries, ['demo']);
  assert.deepEqual(
    rows.filter((r) => r.kind === 'point').map((r) => r.point.text),
    ['a1', 'b1', 'b0', 'a0', 'c0'],
  );
  assert.deepEqual(
    rows.filter((r) => r.kind === 'branch').map((r) => r.branch),
    ['main', 'other'],
  );
});

test('oldest first preserves original point indices and keys for dismissal and motion', () => {
  const newest = viewerRows(entries, ['demo']).filter((r) => r.kind === 'point');
  const oldest = viewerRows(entries, ['demo'], 'submitted-at-asc').filter(
    (r) => r.kind === 'point',
  );
  assert.deepEqual(
    oldest.map((r) => r.point.text),
    ['a0', 'b0', 'b1', 'a1', 'c0'],
  );
  for (const row of oldest) {
    assert.equal(row.entry.points[row.pointIndex], row.point);
    assert.equal(row.key, newest.find((r) => r.point === row.point)?.key);
  }
  assert.deepEqual(
    oldest.filter((r) => r.last).map((r) => r.point.text),
    ['a1', 'c0'],
  );
});

test('equal timestamps keep submitted order and sorting does not mutate entries', () => {
  const tied = [entry('t', 'main', ['2026-11-03T01:00:00Z', '2026-11-03T01:00:00Z'])];
  const before = structuredClone(entries);
  assert.deepEqual(
    viewerRows(tied, ['demo'])
      .filter((r) => r.kind === 'point')
      .map((r) => r.point.text),
    ['t0', 't1'],
  );
  viewerRows(entries, ['demo']);
  assert.deepEqual(entries, before);
});

test('count fade warns at capacity across sessions and branches, separately per project', () => {
  const other = { ...entry('d', 'main', [null]), folder_name: 'other' };
  assert.equal(countExpiryKeys([...entries, other], 6).size, 0);
  assert.deepEqual(
    [...countExpiryKeys([...entries, other], 5)],
    [pointKey(entries[0], entries[0].points[0], 0)],
  );
  assert.equal(countExpiryKeys(entries, null).size, 0);
});

test('count fade includes pending overflow and follows backend tie order with timestamp fallback', () => {
  const tied = [entry('a', 'main', [null, null]), entry('b', 'other', [null])];
  const before = structuredClone(tied);
  assert.deepEqual(
    [...countExpiryKeys(tied, 2)],
    [pointKey(tied[0], tied[0].points[0], 0), pointKey(tied[0], tied[0].points[1], 1)],
  );
  assert.deepEqual(tied, before);
});

test('repository grouping merges directories and sorts points within shared branches', () => {
  const data = [
    { ...entry('a', 'main', ['2026-11-03T01:00:00Z']), repository: 'owner/repo' },
    {
      ...entry('b', 'main', ['2026-11-03T02:00:00Z']),
      folder_name: 'worktree',
      repository: 'owner/repo',
    },
    { ...entry('c', 'feature', [null]), repository: 'owner/repo' },
    { ...entry('d', 'main', [null]), repository: 'other/repo' },
    { ...entry('e', 'main', [null]), folder_name: 'owner/repo' },
  ];
  const normal = viewerRows(data, projectNames(data, 'project-directory-asc'));
  assert.equal(normal.filter((r) => r.kind === 'project').length, 3);
  const rows = viewerRows(
    data,
    projectNames(data, 'project-directory-asc', 'repository'),
    'submitted-at-desc',
    'repository',
  );
  const titles = rows.filter((r) => r.kind === 'project');
  assert.equal(titles.length, 3);
  assert.equal(new Set(titles.map((r) => r.key)).size, 3);
  const repoStart = rows.findIndex((r) => r.kind === 'point' && r.point.text === 'b0') - 2;
  assert(repoStart >= 0);
  assert.deepEqual(
    rows.slice(repoStart + 2, repoStart + 4).map((r) => r.kind === 'point' && r.point.text),
    ['b0', 'a0'],
  );
  assert.equal(rows.filter((r) => r.kind === 'point').length, 5);
  for (const row of rows.filter((r) => r.kind === 'point')) {
    assert.equal(row.entry.points[row.pointIndex], row.point);
    assert(normal.some((other) => other.key === row.key));
  }
  assert.deepEqual(projectNames(data, 'updated-at-desc', 'repository').length, 3);
});

test('missing repositories fall back to separate directories', () => {
  const data = [
    entry('a', 'main', [null]),
    { ...entry('b', 'main', [null]), folder_name: 'second' },
  ];
  const rows = viewerRows(
    data,
    projectNames(data, 'project-directory-asc', 'repository'),
    'submitted-at-desc',
    'repository',
  );
  assert.deepEqual(
    rows.filter((r) => r.kind === 'project').map((r) => r.folder),
    ['demo', 'second'],
  );
  assert.deepEqual(
    rows.filter((r) => r.kind === 'branch').map((r) => r.branch),
    ['main', 'main'],
  );
});

test('repository order uses the latest update across all its directories', () => {
  const data = [
    { ...entry('a', 'main', [null]), repository: 'owner/a', updated_at: '2026-11-01T00:00:00Z' },
    { ...entry('b', 'main', [null]), repository: 'owner/b', updated_at: '2026-11-02T00:00:00Z' },
    {
      ...entry('c', 'main', [null]),
      repository: 'owner/a',
      folder_name: 'worktree',
      updated_at: '2026-11-03T00:00:00Z',
    },
  ];
  const labels = (order: 'updated-at-desc' | 'updated-at-asc' | 'project-directory-asc') =>
    viewerRows(data, projectNames(data, order, 'repository'), 'submitted-at-desc', 'repository')
      .filter((r) => r.kind === 'project')
      .map((r) => r.folder);
  assert.deepEqual(labels('updated-at-desc'), ['owner/a', 'owner/b']);
  assert.deepEqual(labels('updated-at-asc'), ['owner/b', 'owner/a']);
  assert.deepEqual(labels('project-directory-asc'), ['owner/a', 'owner/b']);
});
