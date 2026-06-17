import {spawn} from 'node:child_process';
import {
  CliResolutionError,
  formatResolution,
  formatResolutionAttempts,
  resolveRustCli,
} from './resolve.mjs';

export const packageName = '@mf2-i18n/cli';
export const packageVersion = '0.1.0';

const commandNames = new Set(['build-web', 'check-web', 'doctor']);
const optionSpecs = new Map([
  ['config', {takesValue: true}],
  ['target', {takesValue: true}],
  ['out', {takesValue: true}],
  ['cwd', {takesValue: true}],
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
    'usage: mf2-i18n build-web --target <name> [--config <path>] [--out <dir>] [--cwd <path>] [--check] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n check-web --target <name> [--config <path>] [--out <dir>] [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
    '       mf2-i18n doctor [--cwd <path>] [--cli-bin <path>] [--manifest-path <path>] [--verbose]',
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

  const options = parseOptions(args);
  if (options.help) {
    return {kind: 'help'};
  }

  if ((first === 'build-web' || first === 'check-web') && !options.target) {
    throw new CliUsageError(`${first} requires --target <name>`);
  }

  if (first === 'doctor' && options.target) {
    throw new CliUsageError('doctor does not accept --target');
  }

  return {
    kind: 'command',
    command: first,
    options: {
      config: options.config ?? 'mf2_i18n.toml',
      target: options.target,
      out: options.out,
      cwd: options.cwd,
      cliBin: options['cli-bin'],
      manifestPath: options['manifest-path'],
      check: first === 'check-web' ? true : Boolean(options.check),
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
    if (parsed.command === 'doctor') {
      const resolution = resolveRustCli(parsed.options, runtime);
      io.stdout.write(`${formatResolution(resolution)}\n`);
      return 0;
    }
    const resolution = resolveRustCli(parsed.options, runtime);
    return await runRustCli(resolution, buildWebArgs(parsed.command, parsed.options), io, runtime);
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

export function buildWebArgs(command, options) {
  const args = ['build-web', '--target', options.target, '--config', options.config];
  if (options.out) {
    args.push('--out', options.out);
  }
  if (command === 'check-web' || options.check) {
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
