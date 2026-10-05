// PTY sessions that already had a terminal view. An XtermPane mounting for
// one of these is a remount (zoom toggle, tab moved to another panel), not a
// newly opened terminal.
const shown = new Set<string>();

/** Records the session and reports whether it had been shown before. */
export const markSessionShown = (key: string): boolean => {
  const before = shown.has(key);
  shown.add(key);
  return before;
};
