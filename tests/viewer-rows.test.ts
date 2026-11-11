import test from 'node:test';
import assert from 'node:assert/strict';
import { countExpiryKeys, pointKey, viewerRows } from '../src/viewer/rows.ts';
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

test('header layouts show repository and branch and retain every point', () => {
  const data = entries.map((item) => ({ ...item, repository: 'owner/repo' }));
  const normal = viewerRows(data, ['demo']);
  assert.equal(normal[0].kind === 'project' && normal[0].folder, 'demo');
  assert.equal(normal[1].kind === 'branch' && normal[1].branch, 'owner/repo · main');
  const flipped = viewerRows(
    data,
    ['demo'],
    'submitted-at-desc',
    'repository-branch',
    'project-directory',
  );
  assert.equal(flipped[0].kind === 'project' && flipped[0].folder, 'owner/repo · main');
  assert.equal(flipped[1].kind === 'branch' && flipped[1].branch, 'demo');
  assert.equal(flipped.filter((r) => r.kind === 'project').length, 2);
  assert.deepEqual(
    flipped.filter((r) => r.kind === 'point'),
    normal.filter((r) => r.kind === 'point'),
  );
  const legacy = viewerRows(
    entries,
    ['demo'],
    'submitted-at-desc',
    'repository-branch',
    'project-directory',
  );
  assert.equal(legacy[0].kind === 'project' && legacy[0].folder, 'main');
});

test('title and subtitle are independent, and None omits subtitle rows', () => {
  const data = entries.map((item) => ({ ...item, repository: 'owner/repo' }));
  for (const title of ['project-directory', 'repository-branch'] as const) {
    for (const subtitle of ['project-directory', 'repository-branch', 'none'] as const) {
      const rows = viewerRows(data, ['demo'], 'submitted-at-desc', title, subtitle);
      const titles = rows.filter((row) => row.kind === 'project');
      const subtitles = rows.filter((row) => row.kind === 'branch');
      assert.equal(titles[0].folder, title === 'project-directory' ? 'demo' : 'owner/repo · main');
      if (subtitle === 'none') assert.deepEqual(subtitles, []);
      else {
        assert.equal(subtitles.length, 2);
        assert.equal(
          subtitles[0].branch,
          subtitle === 'project-directory' ? 'demo' : 'owner/repo · main',
        );
      }
      assert.deepEqual(
        rows.filter((row) => row.kind === 'point'),
        viewerRows(data, ['demo']).filter((row) => row.kind === 'point'),
      );
    }
  }
});
