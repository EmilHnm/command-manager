BEGIN;

-- Agent Studio: tasks a person does (manual tests on the UI). The AI
-- prepares them (a test guide), the person reports per case with notes and
-- screenshots, the leader turns failures into fix tasks; `human` holds the
-- guide, the results of each round and the chat with the leader.
ALTER TABLE agent_task ADD COLUMN kind TEXT NOT NULL DEFAULT 'agent';
ALTER TABLE agent_task ADD COLUMN human TEXT;

UPDATE schema_version SET version = 22;

COMMIT;
