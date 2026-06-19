import assert from 'node:assert/strict';
import {chmod, mkdir, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {test} from 'node:test';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import {buildRustArgs, buildWebArgs, packageVersion, parseArgs, runCli} from '../src/cli.mjs';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');

test('package exposes the mf2-i18n bin', async () => {
  const manifest = JSON.parse(await readFile(resolve(packageRoot, 'package.json'), 'utf8'));
  assert.equal(manifest.name, '@mf2-i18n/cli');
  assert.equal(manifest.bin['mf2-i18n'], './bin/mf2-i18n.mjs');
  assert.equal(manifest.type, 'module');
  assert.deepEqual(manifest.files, ['bin', 'src']);
});

test('prints help and version', async () => {
  const help = await capture(['--help']);
  assert.equal(help.exitCode, 0);
  assert.match(help.stdout, /mf2-i18n build --target/);

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
    subcommand: undefined,
    options: {
      config: 'locales/mf2_i18n.toml',
      target: 'docs',
      out: 'src/generated/mf2',
      cwd: 'site',
      preset: undefined,
      sourceLayout: undefined,
      source: undefined,
      locale: undefined,
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

test('parses public command options', () => {
  const build = parseArgs(['build', '--target', 'docs']);
  assert.equal(build.command, 'build');
  assert.equal(build.options.target, 'docs');

  const check = parseArgs(['check', '--target', 'docs']);
  assert.equal(check.command, 'check');
  assert.equal(check.options.check, true);

  const scaffold = parseArgs([
    'scaffold',
    'missing',
    '--source',
    'site',
    '--locale',
    'es',
  ]);
  assert.equal(scaffold.command, 'scaffold');
  assert.equal(scaffold.subcommand, 'missing');
  assert.equal(scaffold.options.source, 'site');
  assert.equal(scaffold.options.locale, 'es');
});

test('builds Rust build-web arguments', () => {
  const project = resolve('/tmp/mf2-i18n-project');

  assert.deepEqual(
    buildWebArgs('build-web', {
      target: 'docs',
      config: 'locales/mf2_i18n.toml',
      out: 'src/generated/mf2',
      check: false,
    }, project),
    [
      'build-web',
      '--target',
      'docs',
      '--config',
      resolve(project, 'locales/mf2_i18n.toml'),
      '--out',
      resolve(project, 'src/generated/mf2'),
    ],
  );

  assert.deepEqual(
    buildWebArgs('check-web', {
      target: 'docs',
      config: 'mf2_i18n.toml',
      check: true,
    }, project),
    ['build-web', '--target', 'docs', '--config', resolve(project, 'mf2_i18n.toml'), '--check'],
  );
});

test('builds Rust public command arguments', () => {
  const project = resolve('/tmp/mf2-i18n-project');

  assert.deepEqual(
    buildRustArgs('build', undefined, {
      target: 'docs',
      config: 'locales/mf2_i18n.toml',
      out: 'src/generated/mf2',
    }, project),
    [
      'build',
      '--target',
      'docs',
      '--config',
      resolve(project, 'locales/mf2_i18n.toml'),
      '--out',
      resolve(project, 'src/generated/mf2'),
    ],
  );

  assert.deepEqual(
    buildRustArgs('check', undefined, {
      target: 'docs',
      config: 'mf2_i18n.toml',
    }, project),
    ['check', '--target', 'docs', '--config', resolve(project, 'mf2_i18n.toml')],
  );

  assert.deepEqual(
    buildRustArgs('init', undefined, {
      config: 'mf2_i18n.toml',
      preset: 'react',
      sourceLayout: 'namespace-locale',
      target: 'app',
    }, project),
    [
      'init',
      '--config',
      resolve(project, 'mf2_i18n.toml'),
      '--preset',
      'react',
      '--source-layout',
      'namespace-locale',
      '--target',
      'app',
    ],
  );

  assert.deepEqual(
    buildRustArgs('scaffold', 'missing', {
      source: 'site',
      locale: 'es',
      config: 'mf2_i18n.toml',
    }, project),
    [
      'scaffold',
      'missing',
      '--source',
      'site',
      '--locale',
      'es',
      '--config',
      resolve(project, 'mf2_i18n.toml'),
    ],
  );
});

test('reports unknown commands and missing targets', async () => {
  const unknown = await capture(['sync-web']);
  assert.equal(unknown.exitCode, 1);
  assert.match(unknown.stderr, /unknown command sync-web/);

  const missingTarget = await capture(['build-web']);
  assert.equal(missingTarget.exitCode, 1);
  assert.match(missingTarget.stderr, /build-web requires --target <name>/);
});

test('published launcher rejects dto type artifact flags', async () => {
  const result = await capture(['build-web', '--target', 'docs', '--dto-types']);
  assert.equal(result.exitCode, 1);
  assert.match(result.stderr, /unknown option --dto-types/);
});

test('doctor invokes an explicit Rust CLI binary', async () => {
  const root = await testRoot('explicit-bin');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);
  const calls = [];

  const result = await capture(
    ['doctor', '--config', 'mf2_i18n.toml', '--cli-bin', bin],
    {cwd: root, spawn: async (call) => recordCall(calls, call, {status: 0})},
  );
  assert.equal(result.exitCode, 0);
  assert.equal(calls[0].command, bin);
  assert.deepEqual(calls[0].args, ['doctor', '--config', resolve(root, 'mf2_i18n.toml')]);
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

  const calls = [];
  const result = await capture(
    ['doctor'],
    {cwd: root, spawn: async (call) => recordCall(calls, call, {status: 0})},
  );
  assert.equal(result.exitCode, 0);
  assert.equal(calls[0].command, bin);
});

test('doctor resolves manifest path through cargo', async () => {
  const root = await testRoot('manifest-path');
  const manifest = resolve(root, 'mf2_i18n/Cargo.toml');
  await mkdir(dirname(manifest), {recursive: true});
  await writeFile(manifest, '[workspace]\n');

  const calls = [];
  const result = await capture(['doctor', '--manifest-path', manifest], {
    cwd: root,
    env: {CARGO: '/custom/cargo'},
    spawn: async (call) => recordCall(calls, call, {status: 0}),
  });
  assert.equal(result.exitCode, 0);
  assert.equal(calls[0].command, '/custom/cargo');
  assert.deepEqual(calls[0].args.slice(0, 6), [
    'run',
    '--manifest-path',
    manifest,
    '-p',
    'mf2_i18n_cli',
    '--',
  ]);
  assert.deepEqual(calls[0].args.slice(6), [
    'doctor',
    '--config',
    resolve(root, 'mf2_i18n.toml'),
  ]);
});

test('doctor resolves an adjacent source checkout', async () => {
  const root = await testRoot('adjacent-source');
  const packageRoot = resolve(root, 'packages/mf2-i18n-cli');
  await mkdir(resolve(packageRoot, 'src'), {recursive: true});
  await mkdir(resolve(root, 'crates/mf2_i18n_cli'), {recursive: true});
  await writeFile(resolve(root, 'Cargo.toml'), '[workspace]\n');
  await writeFile(resolve(root, 'crates/mf2_i18n_cli/Cargo.toml'), '[package]\nname = "mf2_i18n_cli"\n');

  const calls = [];
  const result = await capture(
    ['doctor'],
    {
      cwd: resolve(root, 'site'),
      packageRoot,
      spawn: async (call) => recordCall(calls, call, {status: 0}),
    },
  );
  assert.equal(result.exitCode, 0);
  assert.deepEqual(calls[0].args.slice(-3), [
    'doctor',
    '--config',
    resolve(root, 'site/mf2_i18n.toml'),
  ]);
});

test('doctor reports all failed resolution attempts', async () => {
  const root = await testRoot('missing');
  const result = await capture(['doctor'], {cwd: root, packageRoot: root});
  assert.equal(result.exitCode, 1);
  assert.match(result.stderr, /mf2_i18n_cli could not be resolved/);
  assert.match(result.stderr, /MF2_I18N_CLI_BIN: skipped/);
  assert.match(result.stderr, /adjacent source checkout: skipped/);
});

test('build-web invokes an explicit Rust CLI binary', async () => {
  const root = await testRoot('run-bin');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);
  const calls = [];

  const result = await capture(
    [
      'build-web',
      '--target',
      'docs',
      '--config',
      'locales/mf2_i18n.toml',
      '--out',
      'src/generated/mf2',
      '--cli-bin',
      bin,
    ],
    {cwd: root, spawn: async (call) => recordCall(calls, call, {status: 0})},
  );

  assert.equal(result.exitCode, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, bin);
  assert.equal(calls[0].cwd, root);
  assert.deepEqual(calls[0].args, [
    'build-web',
    '--target',
    'docs',
    '--config',
    resolve(root, 'locales/mf2_i18n.toml'),
    '--out',
    resolve(root, 'src/generated/mf2'),
  ]);
});

test('build and check invoke public Rust commands', async () => {
  const root = await testRoot('run-public');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);
  const calls = [];

  const build = await capture(
    ['build', '--target', 'docs', '--out', 'src/generated/mf2', '--cli-bin', bin],
    {cwd: root, spawn: async (call) => recordCall(calls, call, {status: 0})},
  );
  assert.equal(build.exitCode, 0);

  const check = await capture(
    ['check', '--target', 'docs', '--cli-bin', bin],
    {cwd: root, spawn: async (call) => recordCall(calls, call, {status: 0})},
  );
  assert.equal(check.exitCode, 0);

  assert.deepEqual(calls[0].args, [
    'build',
    '--target',
    'docs',
    '--config',
    resolve(root, 'mf2_i18n.toml'),
    '--out',
    resolve(root, 'src/generated/mf2'),
  ]);
  assert.deepEqual(calls[1].args, [
    'check',
    '--target',
    'docs',
    '--config',
    resolve(root, 'mf2_i18n.toml'),
  ]);
});

test('check-web invokes build-web with check mode', async () => {
  const root = await testRoot('run-check');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);
  const calls = [];

  const result = await capture(
    ['check-web', '--target', 'docs', '--cli-bin', bin],
    {cwd: root, spawn: async (call) => recordCall(calls, call, {status: 0})},
  );

  assert.equal(result.exitCode, 0);
  assert.deepEqual(calls[0].args, [
    'build-web',
    '--target',
    'docs',
    '--config',
    resolve(root, 'mf2_i18n.toml'),
    '--check',
  ]);
});

test('build-web appends arguments after cargo manifest launcher args', async () => {
  const root = await testRoot('run-manifest');
  const manifest = resolve(root, 'mf2_i18n/Cargo.toml');
  await mkdir(dirname(manifest), {recursive: true});
  await writeFile(manifest, '[workspace]\n');
  const calls = [];

  const result = await capture(
    ['build-web', '--target', 'docs', '--manifest-path', manifest],
    {
      cwd: root,
      env: {CARGO: '/custom/cargo'},
      spawn: async (call) => recordCall(calls, call, {status: 0}),
    },
  );

  assert.equal(result.exitCode, 0);
  assert.equal(calls[0].command, '/custom/cargo');
  assert.deepEqual(calls[0].args, [
    'run',
    '--manifest-path',
    manifest,
    '-p',
    'mf2_i18n_cli',
    '--',
    'build-web',
    '--target',
    'docs',
    '--config',
    resolve(root, 'mf2_i18n.toml'),
  ]);
});

test('build-web returns child failure status and stderr', async () => {
  const root = await testRoot('run-failure');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 37\n');
  await chmod(bin, 0o755);

  const result = await capture(
    ['build-web', '--target', 'docs', '--cli-bin', bin],
    {cwd: root, spawn: async () => ({status: 37, stderr: 'build failed\n'})},
  );

  assert.equal(result.exitCode, 37);
  assert.equal(result.stderr, 'build failed\n');
});

test('build-web maps child signals to shell-compatible exit codes', async () => {
  const root = await testRoot('run-signal');
  const bin = resolve(root, 'mf2_i18n_cli');
  await writeFile(bin, '#!/bin/sh\nexit 0\n');
  await chmod(bin, 0o755);

  const result = await capture(
    ['build-web', '--target', 'docs', '--cli-bin', bin],
    {cwd: root, spawn: async () => ({signal: 'SIGINT'})},
  );

  assert.equal(result.exitCode, 130);
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

function recordCall(calls, call, result) {
  calls.push(call);
  return result;
}
