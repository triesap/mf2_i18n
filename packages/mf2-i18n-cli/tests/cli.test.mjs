import assert from 'node:assert/strict';
import {chmod, mkdir, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
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

test('doctor resolves an explicit cli binary', async () => {
  const root = await testRoot('explicit-bin');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);

  const result = await capture(['doctor', '--cli-bin', bin], {cwd: root});
  assert.equal(result.exitCode, 0);
  assert.match(result.stdout, /strategy: cli-bin/);
  assert.match(result.stdout, new RegExp(escapeRegExp(bin)));
});

test('doctor resolves package cliBin config', async () => {
  const root = await testRoot('package-bin');
  const bin = resolve(root, 'vendor/mf2_i18n_cli');
  await mkdir(dirname(bin), {recursive: true});
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);
  await writeFile(
    resolve(root, 'package.json'),
    JSON.stringify({mf2_i18n: {cliBin: './vendor/mf2_i18n_cli'}}, null, 2),
  );

  const result = await capture(['doctor'], {cwd: root});
  assert.equal(result.exitCode, 0);
  assert.match(result.stdout, /strategy: package mf2_i18n\.cliBin/);
});

test('doctor resolves manifest path through cargo', async () => {
  const root = await testRoot('manifest-path');
  const manifest = resolve(root, 'mf2_i18n/Cargo.toml');
  await mkdir(dirname(manifest), {recursive: true});
  await writeFile(manifest, '[workspace]\n');

  const result = await capture(['doctor', '--manifest-path', manifest], {
    cwd: root,
    env: {CARGO: '/custom/cargo'},
  });
  assert.equal(result.exitCode, 0);
  assert.match(result.stdout, /strategy: manifest-path/);
  assert.match(result.stdout, /command: \/custom\/cargo run --manifest-path/);
});

test('doctor resolves an adjacent source checkout', async () => {
  const root = await testRoot('adjacent-source');
  const packageRoot = resolve(root, 'packages/mf2-i18n-cli');
  await mkdir(resolve(packageRoot, 'src'), {recursive: true});
  await mkdir(resolve(root, 'crates/mf2_i18n_cli'), {recursive: true});
  await writeFile(resolve(root, 'Cargo.toml'), '[workspace]\n');
  await writeFile(resolve(root, 'crates/mf2_i18n_cli/Cargo.toml'), '[package]\nname = "mf2_i18n_cli"\n');

  const result = await capture(['doctor'], {cwd: resolve(root, 'site'), packageRoot});
  assert.equal(result.exitCode, 0);
  assert.match(result.stdout, /strategy: adjacent source checkout/);
});

test('doctor reports all failed resolution attempts', async () => {
  const root = await testRoot('missing');
  const result = await capture(['doctor'], {cwd: root, packageRoot: root});
  assert.equal(result.exitCode, 1);
  assert.match(result.stderr, /mf2_i18n_cli could not be resolved/);
  assert.match(result.stderr, /MF2_I18N_CLI_BIN: skipped/);
  assert.match(result.stderr, /adjacent source checkout: skipped/);
});

async function capture(argv, runtime = {}) {
  let stdout = '';
  let stderr = '';
  const exitCode = await runCli(argv, {
    stdout: {write: (value) => { stdout += value; }},
    stderr: {write: (value) => { stderr += value; }},
  }, runtime);
  return {exitCode, stdout, stderr};
}

async function testRoot(name) {
  const root = resolve(
    tmpdir(),
    `mf2_i18n_cli_${name}_${process.pid}_${Date.now()}_${Math.random().toString(16).slice(2)}`,
  );
  await mkdir(root, {recursive: true});
  return root;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
