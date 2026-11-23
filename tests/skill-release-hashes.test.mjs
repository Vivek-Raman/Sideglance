import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { releaseSkillHash } from '../scripts/skill-release-hashes.mjs';

test('release hash matches the Rust fixture and changes with skill contents', () => {
  const original = process.cwd();
  const directory = mkdtempSync(join(tmpdir(), 'sideglance-release-hashes-'));
  const git = (...args) => execFileSync('git', args, { cwd: directory, stdio: 'pipe' });
  try {
    git('init');
    const skill = join(directory, 'src-tauri/resources/skills/sideglance');
    mkdirSync(join(skill, 'nested'), { recursive: true });
    writeFileSync(join(skill, 'SKILL.md'), 'base');
    writeFileSync(join(skill, 'nested/notes.md'), 'notes');
    git('add', '.');
    git('-c', 'user.name=Test', '-c', 'user.email=test@example.com', 'commit', '-m', 'Fixture');
    git('tag', 'v1.0.0');
    process.chdir(directory);
    assert.equal(
      releaseSkillHash('HEAD'),
      '0b3e6a3128080b554b1fc1a3d3a363c21e31b5c18aff4183b2f06afff6fb10ec',
    );
    const old = releaseSkillHash('HEAD');
    writeFileSync(join(skill, 'SKILL.md'), 'updated');
    git('add', '.');
    git('-c', 'user.name=Test', '-c', 'user.email=test@example.com', 'commit', '-m', 'Update');
    git('tag', 'v1.1.0');
    assert.notEqual(releaseSkillHash('HEAD'), old);
  } finally {
    process.chdir(original);
    rmSync(directory, { recursive: true, force: true });
  }
});
