/**
 * Utility functions for formatting and sanitizing file paths dragged and dropped into terminals.
 * Supports modern CLI tools such as Claude Code (claude CLI), Vim, and shells (bash, zsh, powershell, cmd).
 */

/**
 * Checks whether a path contains ASCII/Unicode control characters (0x00-0x1F, 0x7F-0x9F).
 * Prevents command injection and premature bracketed paste termination (\x1b[201~; rm -rf ~\n).
 */
export function hasControlCharacters(path: string): boolean {
  return /[\u0000-\u001f\u007f-\u009f]/.test(path);
}

/**
 * Checks whether the host platform is Windows.
 */
export function isWindowsPlatform(userAgent?: string): boolean {
  const ua = userAgent ?? (typeof navigator !== 'undefined' ? navigator.userAgent : '');
  return /Windows/i.test(ua);
}

/**
 * Formats a file path for safe insertion into the active terminal.
 *
 * Rules:
 * 1. Rejects paths with control characters.
 * 2. Leaves clean paths unquoted when they contain no spaces or special characters.
 * 3. On Windows:
 *    - Uses single quotes '...' (escaping ' as '') for PowerShell when the path contains $ or backtick (`),
 *      preventing variable expansion and escape interpretation.
 *    - Defaults to double quotes "..." (escaping " as "") matching Windows Terminal behavior.
 * 4. On POSIX (Linux / macOS):
 *    - Uses single quotes '...' (escaping ' as '\'') matching GNOME Terminal / VTE behavior.
 */
export function formatDroppedPath(
  filePath: string,
  shellKind?: string,
  userAgent?: string,
): string {
  if (hasControlCharacters(filePath)) {
    throw new Error(`Path contains unsafe control characters: ${filePath}`);
  }

  const needsQuoting = /[\s"'`$!#&*?()[\]{}|;<>~]/.test(filePath);
  if (!needsQuoting) {
    return filePath;
  }

  const isWin = isWindowsPlatform(userAgent);

  if (isWin) {
    const isPwsh = shellKind === 'powershell' || shellKind === 'pwsh';
    if (isPwsh && /[$`]/.test(filePath)) {
      return `'${filePath.replace(/'/g, "''")}'`;
    }
    return `"${filePath.replace(/"/g, '""')}"`;
  }

  return `'${filePath.replace(/'/g, "'\\''")}'`;
}

/**
 * Formats a list of dropped file paths.
 * - Skips paths with unsafe control characters.
 * - Joins valid paths with spaces and appends a single trailing space.
 * - Does NOT append a newline/Enter so users can continue typing.
 */
export function formatDroppedPaths(
  paths: string[],
  shellKind?: string,
  userAgent?: string,
): { text: string; skipped: string[] } {
  const validFormatted: string[] = [];
  const skipped: string[] = [];

  for (const path of paths) {
    if (hasControlCharacters(path)) {
      skipped.push(path);
    } else {
      validFormatted.push(formatDroppedPath(path, shellKind, userAgent));
    }
  }

  const text = validFormatted.length > 0 ? validFormatted.join(' ') + ' ' : '';
  return { text, skipped };
}
