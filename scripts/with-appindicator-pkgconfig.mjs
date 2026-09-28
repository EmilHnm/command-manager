import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const commandArgs = process.argv.slice(2);
const separatorIndex = commandArgs.indexOf('--');
if (separatorIndex >= 0) commandArgs.splice(separatorIndex, 1);

if (commandArgs.length === 0) {
  console.error('Missing command to execute.');
  process.exit(2);
}

const env = { ...process.env };

function commandSucceeded(command, args) {
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    stdio: 'ignore',
    windowsHide: true,
  });
  return !result.error && result.status === 0;
}

function findLinuxLibrary(pattern) {
  const result = spawnSync('ldconfig', ['-p'], {
    cwd: projectRoot,
    encoding: 'utf8',
    windowsHide: true,
  });
  if (result.error || result.status !== 0) return '';
  return (result.stdout || '')
    .split(/\r?\n/)
    .find((line) => pattern.test(line))
    ?.trim()
    .split(/\s+/)
    .at(-1) || '';
}

function writePkgConfigFile(directory, name, libraryPath) {
  const libraryDirectory = dirname(libraryPath);
  writeFileSync(
    resolve(directory, `${name}.pc`),
    [
      `Name: ${name}`,
      'Description: Installed AppIndicator runtime library',
      'Version: 0.5.94',
      `Libs: -L${libraryDirectory} -l${name}`,
      'Cflags:',
      '',
    ].join('\n'),
    'utf8',
  );
}

// AppIndicator pkg-config repair is only needed by the Linux tray build.
// Windows and macOS should invoke the Tauri CLI directly and must not need sh,
// ldconfig, awk, or Linux .pc files.
if (process.platform === 'linux') {
  const hasAyatana = commandSucceeded('pkg-config', ['--exists', 'ayatana-appindicator3-0.1']);
  const hasLegacy = commandSucceeded('pkg-config', ['--exists', 'appindicator3-0.1']);

  if (!hasAyatana && !hasLegacy) {
    const pkgConfigDirectory = resolve(projectRoot, 'src-tauri', 'target', 'pkgconfig');
    mkdirSync(pkgConfigDirectory, { recursive: true });
    const ayatana = findLinuxLibrary(/libayatana-appindicator3\.so\.1\s/);
    const legacy = findLinuxLibrary(/libappindicator3\.so\.1\s/);
    if (ayatana) {
      writePkgConfigFile(pkgConfigDirectory, 'ayatana-appindicator3-0.1', ayatana);
    } else if (legacy) {
      writePkgConfigFile(pkgConfigDirectory, 'appindicator3-0.1', legacy);
    }

    env.PKG_CONFIG_PATH = [pkgConfigDirectory, env.PKG_CONFIG_PATH]
      .filter(Boolean)
      .join(':');
  }
}

const [requestedCommand, ...args] = commandArgs;
const executable = process.platform === 'win32' && requestedCommand === 'tauri'
  ? 'tauri.cmd'
  : requestedCommand;
const result = spawnSync(executable, args, {
  cwd: projectRoot,
  env,
  // npm/pnpm exposes local CLI binaries as .cmd shims on Windows.
  // Running through cmd.exe is required for those shims to work reliably.
  shell: process.platform === 'win32',
  stdio: 'inherit',
  windowsHide: true,
});

if (result.error) {
  console.error(`Failed to start ${requestedCommand}: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
