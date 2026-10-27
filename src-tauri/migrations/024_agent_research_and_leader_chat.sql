BEGIN;

-- Agent Studio: the leader's research before the design (its report, the
-- QA it raised and the answers given at gate 0), JSON; NULL: no research.
ALTER TABLE agent_run ADD COLUMN research TEXT;
-- Questions asked to the run's leader and its answers, JSON.
ALTER TABLE agent_run ADD COLUMN leader_chat TEXT;

UPDATE schema_version SET version = 24;

COMMIT;
