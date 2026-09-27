BEGIN;

-- 006 shipped patterns that were too broad under case-insensitive matching:
-- -p\S+ also matched PowerShell -Path/-Property and ssh -p2222. Only rewrite
-- the shipped default; preserve a user's explicit settings.
UPDATE app_setting
SET value = '(?:password|passwd|pwd|secret|token|api[_-]?key)\w*\s*[=:]\s*\S+
--(?:password|token|secret|api[_-]?key)(?:=|\s+)\S+
\b(?:mysql|mysqldump|mysqladmin)\b.*\s(?-i:-p)(?:\S+|\s+\S+)
authorization:\s*\S+
\bbearer\s+\S+
://[^/\s:@]+:[^@\s]+@'
WHERE key = 'history_block_patterns'
  AND value = 'password=
token
-p\S+
authorization:';

UPDATE schema_version SET version = 7;

COMMIT;
