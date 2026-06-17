import {existsSync, readFileSync} from 'node:fs';
import {dirname, isAbsolute, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const defaultPackageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export class CliResolutionError extends Error {
  constructor(attempts) {
    super('mf2_i18n_cli could not be resolved');
    this.name = 'CliResolutionError';
    this.attempts = attempts;
  }
}

export function resolveRustCli(options = {}, runtime = {}) {
  const env = runtime.env ?? process.env;
  const packageRoot = runtime.packageRoot ?? defaultPackageRoot;
  const cwd = resolve(runtime.cwd ?? process.cwd(), options.cwd ?? '.');
  const attempts = [];
  const packageConfig = readNearestPackageConfig(cwd);

  const cliBinCandidates = [
    ['cli-bin', options.cliBin],
    ['MF2_I18N_CLI_BIN', env.MF2_I18N_CLI_BIN],
    ['package mf2_i18n.cliBin', packageConfig?.config?.cliBin],
    ['package native binary', nativeBinaryPath(packageRoot)],
  ];

  for (const [strategy, value] of cliBinCandidates) {
    if (!value) {
      attempts.push({strategy, status: 'skipped'});
      continue;
    }

    const path = resolveFrom(cwd, value);
    if (!existsSync(path)) {
      attempts.push({strategy, status: 'missing', path});
      continue;
    }

    attempts.push({strategy, status: 'selected', path});
    return {
      strategy,
      command: path,
      args: [],
      cwd,
      projectCwd: cwd,
      attempts,
    };
  }

  const manifestCandidates = [
    ['manifest-path', options.manifestPath],
    ['MF2_I18N_CARGO_MANIFEST_PATH', env.MF2_I18N_CARGO_MANIFEST_PATH],
    ['package mf2_i18n.cargoManifestPath', packageConfig?.config?.cargoManifestPath],
    ['adjacent source checkout', adjacentSourceManifest(packageRoot)],
  ];

  for (const [strategy, value] of manifestCandidates) {
    if (!value) {
      attempts.push({strategy, status: 'skipped'});
      continue;
    }

    const manifestPath = resolveFrom(cwd, value);
    if (!existsSync(manifestPath)) {
      attempts.push({strategy, status: 'missing', path: manifestPath});
      continue;
    }

    attempts.push({strategy, status: 'selected', path: manifestPath});
    return {
      strategy,
      command: env.CARGO ?? 'cargo',
      args: ['run', '--manifest-path', manifestPath, '-p', 'mf2_i18n_cli', '--'],
      cwd: dirname(manifestPath),
      projectCwd: cwd,
      attempts,
    };
  }

  throw new CliResolutionError(attempts);
}

export function formatResolution(resolution) {
  const args = resolution.args.length > 0 ? ` ${resolution.args.join(' ')}` : '';
  return [
    `strategy: ${resolution.strategy}`,
    `command: ${resolution.command}${args}`,
    `cwd: ${resolution.cwd}`,
    `project cwd: ${resolution.projectCwd}`,
  ].join('\n');
}

export function formatResolutionAttempts(attempts) {
  return attempts
    .map((attempt) => {
      if (attempt.status === 'skipped') {
        return `- ${attempt.strategy}: skipped`;
      }
      return `- ${attempt.strategy}: ${attempt.status} ${attempt.path}`;
    })
    .join('\n');
}

function readNearestPackageConfig(startDir) {
  let current = resolve(startDir);
  while (true) {
    const packageJsonPath = resolve(current, 'package.json');
    if (existsSync(packageJsonPath)) {
      const manifest = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
      const config = manifest.mf2_i18n;
      if (config && typeof config === 'object') {
        return {path: packageJsonPath, config};
      }
      return {path: packageJsonPath, config: {}};
    }

    const parent = dirname(current);
    if (parent === current) {
      return undefined;
    }
    current = parent;
  }
}

function nativeBinaryPath(packageRoot) {
  const executable = process.platform === 'win32' ? 'mf2_i18n_cli.exe' : 'mf2_i18n_cli';
  return resolve(packageRoot, 'native', `${process.platform}-${process.arch}`, executable);
}

function adjacentSourceManifest(packageRoot) {
  const candidate = resolve(packageRoot, '..', '..', 'Cargo.toml');
  const cliCrate = resolve(packageRoot, '..', '..', 'crates', 'mf2_i18n_cli', 'Cargo.toml');
  if (existsSync(candidate) && existsSync(cliCrate)) {
    return candidate;
  }
  return undefined;
}

function resolveFrom(cwd, value) {
  return isAbsolute(value) ? value : resolve(cwd, value);
}
