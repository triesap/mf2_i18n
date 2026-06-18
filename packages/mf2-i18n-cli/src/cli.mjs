import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {
  CliResolutionError,
  formatResolutionAttempts,
  resolveRustCli,
} from './resolve.mjs';

export const packageName = '@mf2-i18n/cli';
export const packageVersion = '0.1.0';

const commandNames = new Set([
  'init',
  'detect',
  'build',
  'check',
  'scaffold',
  'doctor',
  'build-web',
  'check-web',
]);
const optionSpecs = new Map([
  ['config', {takesValue: true}],
  ['target', {takesValue: true}],
  ['out', {takesValue: true}],
  ['cwd', {takesValue: true}],
  ['preset', {takesValue: true}],
  ['source-layout', {takesValue: true}],
  ['source', {takesValue: true}],
  ['locale', {takesValue: true}],
  ['cli-bin', {takesValue: true}],
  ['manifest-path', {takesValue: true}],
  ['check', {takesValue: false}],
  ['verbose', {takesValue: false}],
]);

export class CliUsageError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CliUsageError';
  }
}

export function usage() {
  return [
    'usage: mf2-i18n init [--preset <name>] [--source-layout <name>] [--target <name>] [--config <path>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n detect [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n build --target <name> [--config <path>] [--out <dir>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n check --target <name> [--config <path>] [--out <dir>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n scaffold missing --source <name> --locale <locale> [--config <path>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n doctor [--config <path>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    'usage: mf2-i18n build-web --target <name> [--config <path>] [--out <dir>] [--cwd <path>] [--check] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n check-web --target <name> [--config <path>] [--out <dir>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n --version',
    '       mf2-i18n --help',
  ].join('\n');
}

export function parseArgs(argv) {
  const args = [...argv];
  const first = args.shift();

  if (!first || first === '--help' || first === '-h') {
    return {kind: 'help'};
  }

  if (first === '--version' || first === '-V') {
    return {kind: 'version'};
  }

  if (!commandNames.has(first)) {
    throw new CliUsageError(`unknown command ${first}`);
  }

  let subcommand;
  if (first === 'scaffold') {
    subcommand = args.shift();
    if (subcommand !== 'missing') {
      throw new CliUsageError('scaffold requires missing');
    }
  }

  const options = parseOptions(args);
  if (options.help) {
    return {kind: 'help'};
  }

  if (['build', 'check', 'build-web', 'check-web'].includes(first) && !options.target) {
    throw new CliUsageError(`${first} requires --target <name>`);
  }

  if (first === 'scaffold' && !options.source) {
    throw new CliUsageError('scaffold missing requires --source <name>');
  }

  if (first === 'scaffold' && !options.locale) {
    throw new CliUsageError('scaffold missing requires --locale <locale>');
  }

  return {
    kind: 'command',
    command: first,
    subcommand,
    options: {
      config: options.config ?? 'mf2_i18n.toml',
      target: options.target,
      out: options.out,
      cwd: options.cwd,
      preset: options.preset,
      sourceLayout: options['source-layout'],
      source: options.source,
      locale: options.locale,
      cliBin: options['cli-bin'],
      manifestPath: options['manifest-path'],
      check: first === 'check' || first === 'check-web' ? true : Boolean(options.check),
      verbose: Boolean(options.verbose),
    },
  };
}

export async function runCli(argv, io, runtime = {}) {
  try {
    const parsed = parseArgs(argv);
    if (parsed.kind === 'help') {
      io.stdout.write(`${usage()}\n`);
      return 0;
    }
    if (parsed.kind === 'version') {
      io.stdout.write(`${packageVersion}\n`);
      return 0;
    }
    const resolution = resolveRustCli(parsed.options, runtime);
    return await runRustCli(
      resolution,
      buildRustArgs(parsed.command, parsed.subcommand, parsed.options, resolution.projectCwd),
      io,
      runtime,
    );
  } catch (error) {
    if (error instanceof CliUsageError) {
      io.stderr.write(`${error.message}\n\n${usage()}\n`);
      return 1;
    }
    if (error instanceof CliResolutionError) {
      io.stderr.write(`${error.message}\n${formatResolutionAttempts(error.attempts)}\n`);
      return 1;
    }
    throw error;
  }
}

export function buildWebArgs(command, options, projectCwd = process.cwd()) {
  return buildRustArgs(command, undefined, options, projectCwd);
}

export function buildRustArgs(command, subcommand, options, projectCwd = process.cwd()) {
  if (command === 'init') {
    const args = ['init', '--config', resolve(projectCwd, options.config)];
    if (options.preset) {
      args.push('--preset', options.preset);
    }
    if (options.sourceLayout) {
      args.push('--source-layout', options.sourceLayout);
    }
    if (options.target) {
      args.push('--target', options.target);
    }
    return args;
  }

  if (command === 'detect') {
    return ['detect', '--cwd', projectCwd];
  }

  if (command === 'doctor') {
    return ['doctor', '--config', resolve(projectCwd, options.config)];
  }

  if (command === 'scaffold' && subcommand === 'missing') {
    return [
      'scaffold',
      'missing',
      '--source',
      options.source,
      '--locale',
      options.locale,
      '--config',
      resolve(projectCwd, options.config),
    ];
  }

  const args = [
    command === 'build-web' || command === 'check-web' ? 'build-web' : command,
    '--target',
    options.target,
    '--config',
    resolve(projectCwd, options.config),
  ];
  if (options.out) {
    args.push('--out', resolve(projectCwd, options.out));
  }
  if (command === 'check-web' || (command === 'build-web' && options.check)) {
    args.push('--check');
  }
  return args;
}

async function runRustCli(resolution, rustArgs, io, runtime) {
  const command = resolution.command;
  const args = [...resolution.args, ...rustArgs];
  const env = runtime.env ?? process.env;

  if (runtime.spawn) {
    return childExitCode(await runtime.spawn({command, args, cwd: resolution.cwd, env}), io);
  }

  return await new Promise((resolveExitCode) => {
    const child = spawn(command, args, {
      cwd: resolution.cwd,
      env,
      stdio: ['ignore', 'inherit', 'inherit'],
    });

    child.on('error', (error) => {
      io.stderr.write(`failed to start ${command}: ${error.message}\n`);
      resolveExitCode(1);
    });

    child.on('exit', (code, signal) => {
      resolveExitCode(childExitCode({status: code, signal}, io));
    });
  });
}

function childExitCode(result, io) {
  if (typeof result === 'number') {
    return result;
  }
  if (typeof result?.stdout === 'string') {
    io.stdout.write(result.stdout);
  }
  if (typeof result?.stderr === 'string') {
    io.stderr.write(result.stderr);
  }
  if (typeof result?.status === 'number') {
    return result.status;
  }
  if (typeof result?.code === 'number') {
    return result.code;
  }
  if (result?.signal) {
    return 128 + signalNumber(result.signal);
  }
  return 0;
}

function signalNumber(signal) {
  const signals = {
    SIGHUP: 1,
    SIGINT: 2,
    SIGQUIT: 3,
    SIGILL: 4,
    SIGTRAP: 5,
    SIGABRT: 6,
    SIGBUS: 7,
    SIGFPE: 8,
    SIGKILL: 9,
    SIGUSR1: 10,
    SIGSEGV: 11,
    SIGUSR2: 12,
    SIGPIPE: 13,
    SIGALRM: 14,
    SIGTERM: 15,
  };
  return signals[signal] ?? 1;
}

function parseOptions(args) {
  const options = {};

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--help' || arg === '-h') {
      options.help = true;
      continue;
    }
    if (!arg.startsWith('--')) {
      throw new CliUsageError(`unexpected argument ${arg}`);
    }

    const name = arg.slice(2);
    const spec = optionSpecs.get(name);
    if (!spec) {
      throw new CliUsageError(`unknown option --${name}`);
    }

    if (!spec.takesValue) {
      options[name] = true;
      continue;
    }

    const value = args[index + 1];
    if (!value || value.startsWith('--')) {
      throw new CliUsageError(`--${name} requires a value`);
    }
    options[name] = value;
    index += 1;
  }

  return options;
}
