#!/usr/bin/env node
import {runCli} from '../src/cli.mjs';

const exitCode = await runCli(process.argv.slice(2), {
  stdout: process.stdout,
  stderr: process.stderr,
});

process.exit(exitCode);
