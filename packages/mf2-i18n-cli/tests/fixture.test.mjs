import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {cp, mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, resolve} from 'node:path';
import {test} from 'node:test';
import {fileURLToPath} from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(packageRoot, '..', '..');
const binPath = resolve(packageRoot, 'bin/mf2-i18n.mjs');
const fixtureRoot = resolve(packageRoot, 'fixtures/plain-site');
const manifestPath = resolve(repoRoot, 'Cargo.toml');

test('plain site fixture builds and checks through the source checkout launcher', async () => {
  const workRoot = await mkdtemp(resolve(tmpdir(), 'mf2_i18n_plain_site_'));
  await cp(fixtureRoot, workRoot, {recursive: true});

  try {
    runLauncher([
      'build',
      '--cwd',
      workRoot,
      '--config',
      'mf2_i18n.toml',
      '--target',
      'plain-site',
      '--manifest-path',
      manifestPath,
    ]);

    const english = JSON.parse(
      await readFile(resolve(workRoot, 'generated/mf2/messages/en/home.json'), 'utf8'),
    );
    const spanish = JSON.parse(
      await readFile(resolve(workRoot, 'generated/mf2/messages/es/home.json'), 'utf8'),
    );
    assert.equal(english.home.title, 'Hello from MF2');
    assert.equal(spanish.home.title, 'Hola desde MF2');

    await readFile(resolve(workRoot, 'generated/mf2/mf2-web-manifest.json'), 'utf8');
    await readFile(resolve(workRoot, 'generated/mf2/types.ts'), 'utf8');

    runLauncher([
      'check',
      '--cwd',
      workRoot,
      '--config',
      'mf2_i18n.toml',
      '--target',
      'plain-site',
      '--manifest-path',
      manifestPath,
    ]);
  } finally {
    await rm(workRoot, {recursive: true, force: true});
  }
});

function runLauncher(args) {
  const result = spawnSync(process.execPath, [binPath, ...args], {
    cwd: repoRoot,
    encoding: 'utf8',
  });

  assert.equal(
    result.status,
    0,
    [
      `launcher failed with status ${result.status}`,
      `stdout:\n${result.stdout}`,
      `stderr:\n${result.stderr}`,
    ].join('\n'),
  );
}
