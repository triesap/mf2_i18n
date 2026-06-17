import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {test} from 'node:test';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import {packageVersion, parseArgs, runCli} from '../src/cli.mjs';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');

test('package exposes the mf2-i18n bin', async () => {
  const manifest = JSON.parse(await readFile(resolve(packageRoot, 'package.json'), 'utf8'));
  assert.equal(manifest.name, '@mf2-i18n/cli');
  assert.equal(manifest.bin['mf2-i18n'], './bin/mf2-i18n.mjs');
  assert.equal(manifest.type, 'module');
});

test('prints help and version', async () => {
  const help = await capture(['--help']);
  assert.equal(help.exitCode, 0);
  assert.match(help.stdout, /mf2-i18n build-web/);

  const version = await capture(['--version']);
  assert.equal(version.exitCode, 0);
  assert.equal(version.stdout.trim(), packageVersion);
});

test('parses build-web options', () => {
  const parsed = parseArgs([
    'build-web',
    '--target',
    'docs',
    '--config',
    'locales/mf2_i18n.toml',
    '--out',
    'src/generated/mf2',
    '--cwd',
    'site',
    '--check',
    '--cli-bin',
    './bin/mf2_i18n_cli',
    '--manifest-path',
    './mf2_i18n/Cargo.toml',
    '--verbose',
  ]);

  assert.deepEqual(parsed, {
    kind: 'command',
    command: 'build-web',
    options: {
      config: 'locales/mf2_i18n.toml',
      target: 'docs',
      out: 'src/generated/mf2',
      cwd: 'site',
      cliBin: './bin/mf2_i18n_cli',
      manifestPath: './mf2_i18n/Cargo.toml',
      check: true,
      verbose: true,
    },
  });
});

test('check-web enables check mode', () => {
  const parsed = parseArgs(['check-web', '--target', 'docs']);
  assert.equal(parsed.command, 'check-web');
  assert.equal(parsed.options.config, 'mf2_i18n.toml');
  assert.equal(parsed.options.check, true);
});

test('reports unknown commands and missing targets', async () => {
  const unknown = await capture(['sync-web']);
  assert.equal(unknown.exitCode, 1);
  assert.match(unknown.stderr, /unknown command sync-web/);

  const missingTarget = await capture(['build-web']);
  assert.equal(missingTarget.exitCode, 1);
  assert.match(missingTarget.stderr, /build-web requires --target <name>/);
});

async function capture(argv) {
  let stdout = '';
  let stderr = '';
  const exitCode = await runCli(argv, {
    stdout: {write: (value) => { stdout += value; }},
    stderr: {write: (value) => { stderr += value; }},
  });
  return {exitCode, stdout, stderr};
}
