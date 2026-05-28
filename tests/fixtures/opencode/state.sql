-- opencode SQLite state schema (as of v1.14.x).
-- This fixture captures the real session and part table shapes found in
-- ~/.local/share/opencode/opencode.db on a live system.
--
-- The adapter reads:
--   SELECT id, directory, title, parent_id, time_updated, time_created FROM session
-- and the part table for message previews.

CREATE TABLE session (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL DEFAULT 'global',
    parent_id TEXT,
    slug TEXT NOT NULL,
    directory TEXT NOT NULL,
    title TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '1.14.19',
    time_created INTEGER NOT NULL,
    time_updated INTEGER NOT NULL,
    cost REAL NOT NULL DEFAULT 0,
    tokens_input INTEGER NOT NULL DEFAULT 0,
    tokens_output INTEGER NOT NULL DEFAULT 0
);

INSERT INTO session (id, project_id, slug, directory, title, time_created, time_updated)
VALUES ('ses_ffff1111aaaa2222bbbb33333333333', 'global', 'quiet-forest',
        '/home/user/projects/my-project', 'Review project structure',
        1700000000000, 1700000100000);

INSERT INTO session (id, project_id, parent_id, slug, directory, title, time_created, time_updated)
VALUES ('ses_ffff2222aaaa2222bbbb33333333333', 'global',
        'ses_ffff1111aaaa2222bbbb33333333333', 'bold-mountain',
        '/home/user/projects/my-project', 'Follow-up review',
        1700000200000, 1700000300000);

CREATE TABLE part (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    time_created INTEGER NOT NULL,
    time_updated INTEGER NOT NULL,
    data TEXT NOT NULL
);

INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
VALUES ('prt_a001', 'msg_a001', 'ses_ffff1111aaaa2222bbbb33333333333',
        1700000001000, 1700000001000, '{"type":"text","text":"Let me review the project structure."}');

INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
VALUES ('prt_a002', 'msg_a002', 'ses_ffff1111aaaa2222bbbb33333333333',
        1700000010000, 1700000010000, '{"type":"text","text":"The project is well-organized."}');
