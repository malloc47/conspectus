-- opencode 1.18.32 database after opencode 2.0.32 migrated it on first start.
-- The 1.x session/message/part tables stay behind, unchanged; session_v2 and
-- session_message hold copies of the same sessions under the same ids.
-- Captured from sandboxed installs and sanitized: paths rewritten to
-- /home/user, unrelated tables dropped.

CREATE TABLE `project` (
          `id` text PRIMARY KEY,
          `worktree` text NOT NULL,
          `vcs` text,
          `name` text,
          `icon_url` text,
          `icon_url_override` text,
          `icon_color` text,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `time_initialized` integer,
          `sandboxes` text NOT NULL,
          `commands` text
        , `time_active` integer DEFAULT 0 NOT NULL, `time_archived` integer);
INSERT INTO project (id, worktree, vcs, name, icon_url, icon_url_override, icon_color, time_created, time_updated, time_initialized, sandboxes, commands, time_active, time_archived) VALUES ('d9af7d98dcc10e194f77fde4b8c443c5ef1ef635', '/home/user/proj', 'git', NULL, NULL, NULL, NULL, 1792301208902, 1792301814909, NULL, '[]', NULL, 1792302037666, NULL);
INSERT INTO project (id, worktree, vcs, name, icon_url, icon_url_override, icon_color, time_created, time_updated, time_initialized, sandboxes, commands, time_active, time_archived) VALUES ('global', '/', NULL, NULL, NULL, NULL, NULL, 1792302037638, 1792302037638, NULL, '[]', NULL, 1792302037638, NULL);

CREATE TABLE `session` (
          `id` text PRIMARY KEY,
          `project_id` text NOT NULL,
          `workspace_id` text,
          `parent_id` text,
          `slug` text NOT NULL,
          `directory` text NOT NULL,
          `path` text,
          `title` text NOT NULL,
          `version` text NOT NULL,
          `share_url` text,
          `summary_additions` integer,
          `summary_deletions` integer,
          `summary_files` integer,
          `summary_diffs` text,
          `metadata` text,
          `cost` real DEFAULT 0 NOT NULL,
          `tokens_input` integer DEFAULT 0 NOT NULL,
          `tokens_output` integer DEFAULT 0 NOT NULL,
          `tokens_reasoning` integer DEFAULT 0 NOT NULL,
          `tokens_cache_read` integer DEFAULT 0 NOT NULL,
          `tokens_cache_write` integer DEFAULT 0 NOT NULL,
          `revert` text,
          `permission` text,
          `agent` text,
          `model` text,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `time_compacting` integer,
          `time_archived` integer,
          CONSTRAINT `fk_session_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
        );
CREATE INDEX `session_parent_idx` ON `session` (`parent_id`);
CREATE INDEX `session_project_idx` ON `session` (`project_id`);
CREATE INDEX `session_workspace_idx` ON `session` (`workspace_id`);
INSERT INTO session (id, project_id, workspace_id, parent_id, slug, directory, path, title, version, share_url, summary_additions, summary_deletions, summary_files, summary_diffs, metadata, cost, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, revert, permission, agent, model, time_created, time_updated, time_compacting, time_archived) VALUES ('ses_eb282f773ffeudkJBYBQedCh1M', 'd9af7d98dcc10e194f77fde4b8c443c5ef1ef635', NULL, NULL, 'glowing-sailor', '/home/user/proj', '', 'Hello from the mock model.', '1.18.32', NULL, 0, 0, 0, NULL, NULL, 0.0, 10, 5, 0, 0, 0, NULL, '[{"permission":"question","pattern":"*","action":"deny"},{"permission":"plan_enter","pattern":"*","action":"deny"},{"permission":"plan_exit","pattern":"*","action":"deny"}]', 'build', '{"id":"mock-model","providerID":"mock","variant":"default"}', 1792301402252, 1792301403202, NULL, NULL);
INSERT INTO session (id, project_id, workspace_id, parent_id, slug, directory, path, title, version, share_url, summary_additions, summary_deletions, summary_files, summary_diffs, metadata, cost, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, revert, permission, agent, model, time_created, time_updated, time_compacting, time_archived) VALUES ('ses_eb27ccda4ffeKjSodHyeONASJp', 'd9af7d98dcc10e194f77fde4b8c443c5ef1ef635', NULL, NULL, 'neon-otter', '/home/user/proj', '', 'Hello from the mock model.', '1.18.32', NULL, 0, 0, 0, NULL, NULL, 0.0, 20, 10, 0, 0, 0, NULL, '[{"permission":"question","pattern":"*","action":"deny"},{"permission":"plan_enter","pattern":"*","action":"deny"},{"permission":"plan_exit","pattern":"*","action":"deny"}]', 'build', '{"id":"mock-model","providerID":"mock","variant":"default"}', 1792301806171, 1792301807216, NULL, NULL);

CREATE TABLE `message` (
          `id` text PRIMARY KEY,
          `session_id` text NOT NULL,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `data` text NOT NULL,
          CONSTRAINT `fk_message_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
        );
CREATE INDEX `message_session_time_created_id_idx` ON `message` (`session_id`,`time_created`,`id`);
INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES ('msg_14d7d08b500137rkKb3DR11HiA', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301402293, 1792301402763, '{"role":"user","time":{"created":1792301402293},"agent":"build","model":{"providerID":"mock","modelID":"mock-model"},"summary":{"diffs":[]}}');
INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES ('msg_14d7d0a1b001WNmArlt4cjYWh6', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301402651, 1792301403196, '{"parentID":"msg_14d7d08b500137rkKb3DR11HiA","role":"assistant","mode":"build","agent":"build","path":{"cwd":"/home/user/proj","root":"/home/user/proj"},"cost":0,"tokens":{"total":15,"input":10,"output":5,"reasoning":0,"cache":{"write":0,"read":0}},"modelID":"mock-model","providerID":"mock","time":{"created":1792301402651,"completed":1792301403194},"finish":"stop"}');
INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES ('msg_14d83328a001RWfewdJ6XwNujl', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301806218, 1792301807178, '{"role":"user","time":{"created":1792301806218},"agent":"build","model":{"providerID":"mock","modelID":"mock-model"},"summary":{"diffs":[]}}');
INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES ('msg_14d8333e6001Ek0TzWXBMYcsmG', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301806566, 1792301807117, '{"parentID":"msg_14d83328a001RWfewdJ6XwNujl","role":"assistant","mode":"build","agent":"build","path":{"cwd":"/home/user/proj","root":"/home/user/proj"},"cost":0,"tokens":{"total":15,"input":10,"output":5,"reasoning":0,"cache":{"write":0,"read":0}},"modelID":"mock-model","providerID":"mock","time":{"created":1792301806566,"completed":1792301807116},"finish":"tool-calls"}');
INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES ('msg_14d83360f0013b99TySrjFGXQ6', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807119, 1792301807211, '{"parentID":"msg_14d83328a001RWfewdJ6XwNujl","role":"assistant","mode":"build","agent":"build","path":{"cwd":"/home/user/proj","root":"/home/user/proj"},"cost":0,"tokens":{"total":15,"input":10,"output":5,"reasoning":0,"cache":{"write":0,"read":0}},"modelID":"mock-model","providerID":"mock","time":{"created":1792301807119,"completed":1792301807210},"finish":"stop"}');

CREATE TABLE `part` (
          `id` text PRIMARY KEY,
          `message_id` text NOT NULL,
          `session_id` text NOT NULL,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `data` text NOT NULL,
          CONSTRAINT `fk_part_message_id_message_id_fk` FOREIGN KEY (`message_id`) REFERENCES `message`(`id`) ON DELETE CASCADE
        );
CREATE INDEX `part_message_id_id_idx` ON `part` (`message_id`,`id`);
CREATE INDEX `part_session_idx` ON `part` (`session_id`);
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d7d08bd001VkUc9BCWp2tnH0', 'msg_14d7d08b500137rkKb3DR11HiA', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301402640, 1792301402642, '{"type":"text","text":"Say hello"}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d7d0c0d001MZyOCcf73YtviV', 'msg_14d7d0a1b001WNmArlt4cjYWh6', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301403149, 1792301403150, '{"snapshot":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","type":"step-start"}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d7d0c0f0018Rpu46OO7KLFae', 'msg_14d7d0a1b001WNmArlt4cjYWh6', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301403151, 1792301403154, '{"type":"reasoning","text":"The user wants a short greeting.","time":{"start":1792301403151,"end":1792301403154}}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d7d0c13001QVEhwNlNO0e1hk', 'msg_14d7d0a1b001WNmArlt4cjYWh6', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301403155, 1792301403158, '{"type":"text","text":"Hello from the mock model.","time":{"start":1792301403155,"end":1792301403157}}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d7d0c27001bS6My681E7bGZW', 'msg_14d7d0a1b001WNmArlt4cjYWh6', 'ses_eb282f773ffeudkJBYBQedCh1M', 1792301403175, 1792301403177, '{"reason":"stop","snapshot":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","type":"step-finish","tokens":{"total":15,"input":10,"output":5,"reasoning":0,"cache":{"write":0,"read":0}},"cost":0}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d833290001xHnaXZ64iqomLO', 'msg_14d83328a001RWfewdJ6XwNujl', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301806556, 1792301806558, '{"type":"text","text":"Please TOOL /home/user/proj/README.md"}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d8335c5001Lm2C8bSihY6jJU', 'msg_14d8333e6001Ek0TzWXBMYcsmG', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807045, 1792301807046, '{"snapshot":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","type":"step-start"}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d8335c70011FmgIwF6ZJc6Cu', 'msg_14d8333e6001Ek0TzWXBMYcsmG', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807047, 1792301807050, '{"type":"reasoning","text":"I should use a tool first.","time":{"start":1792301807047,"end":1792301807049}}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d8335cb001byLsGJ71sMuxqX', 'msg_14d8333e6001Ek0TzWXBMYcsmG', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807051, 1792301807073, '{"type":"tool","tool":"read","callID":"call_mock_1","state":{"status":"completed","input":{"filePath":"/home/user/proj/README.md"},"output":"<path>/home/user/proj/README.md</path>\n<type>file</type>\n<content>\n1: hello from the fixture\n\n(End of file - total 1 lines)\n</content>","metadata":{"preview":"hello from the fixture","truncated":false,"loaded":[],"display":{"type":"file","path":"/home/user/proj/README.md","text":"hello from the fixture","lineStart":1,"lineEnd":1,"totalLines":1,"truncated":false}},"title":"README.md","time":{"start":1792301807057,"end":1792301807072}}}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d8335f5001HcijJp0uzm76h5', 'msg_14d8333e6001Ek0TzWXBMYcsmG', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807093, 1792301807095, '{"reason":"tool-calls","snapshot":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","type":"step-finish","tokens":{"total":15,"input":10,"output":5,"reasoning":0,"cache":{"write":0,"read":0}},"cost":0}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d833643001E2i4C49VQQQIFF', 'msg_14d83360f0013b99TySrjFGXQ6', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807171, 1792301807172, '{"snapshot":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","type":"step-start"}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d8336460015LkV0W10RKlp7P', 'msg_14d83360f0013b99TySrjFGXQ6', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807174, 1792301807176, '{"type":"text","text":"I read the file; it says hello from the fixture.","time":{"start":1792301807174,"end":1792301807176}}');
INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES ('prt_14d833657001ZX2l73t45Ge8x7', 'msg_14d83360f0013b99TySrjFGXQ6', 'ses_eb27ccda4ffeKjSodHyeONASJp', 1792301807192, 1792301807193, '{"reason":"stop","snapshot":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","type":"step-finish","tokens":{"total":15,"input":10,"output":5,"reasoning":0,"cache":{"write":0,"read":0}},"cost":0}');

CREATE TABLE `session_v2` (
          `id` text PRIMARY KEY,
          `project_id` text NOT NULL,
          `workspace_id` text,
          `parent_id` text,
          `fork_session_id` text,
          `fork_boundary` text,
          `slug` text NOT NULL,
          `directory` text NOT NULL,
          `path` text,
          `title` text,
          `version` text NOT NULL,
          `share_url` text,
          `summary_additions` integer,
          `summary_deletions` integer,
          `summary_files` integer,
          `summary_diffs` text,
          `metadata` text,
          `cost` real DEFAULT 0 NOT NULL,
          `tokens_input` integer DEFAULT 0 NOT NULL,
          `tokens_output` integer DEFAULT 0 NOT NULL,
          `tokens_reasoning` integer DEFAULT 0 NOT NULL,
          `tokens_cache_read` integer DEFAULT 0 NOT NULL,
          `tokens_cache_write` integer DEFAULT 0 NOT NULL,
          `revert` text,
          `permission` text,
          `agent` text,
          `model` text,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `time_compacting` integer,
          `time_archived` integer,
          `time_suspended` integer, `resume_attempts` integer DEFAULT 0 NOT NULL, `time_idle` integer, `time_viewed` integer, `idle_outcome` text,
          CONSTRAINT `fk_session_v2_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
        );
CREATE INDEX `session_v2_parent_idx` ON `session_v2` (`parent_id`);
CREATE INDEX `session_v2_project_idx` ON `session_v2` (`project_id`);
CREATE INDEX `session_v2_time_suspended_idx` ON `session_v2` (`time_suspended`) WHERE "session_v2"."time_suspended" is not null;
CREATE INDEX `session_v2_workspace_idx` ON `session_v2` (`workspace_id`);
INSERT INTO session_v2 (id, project_id, workspace_id, parent_id, fork_session_id, fork_boundary, slug, directory, path, title, version, share_url, summary_additions, summary_deletions, summary_files, summary_diffs, metadata, cost, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, revert, permission, agent, model, time_created, time_updated, time_compacting, time_archived, time_suspended, resume_attempts, time_idle, time_viewed, idle_outcome) VALUES ('ses_eb282f773ffeudkJBYBQedCh1M', 'd9af7d98dcc10e194f77fde4b8c443c5ef1ef635', NULL, NULL, NULL, NULL, 'glowing-sailor', '/home/user/proj', '', 'Hello from the mock model.', '1.18.32', NULL, 0, 0, 0, NULL, NULL, 0.0, 10, 5, 0, 0, 0, NULL, NULL, 'build', '{"id":"mock-model","providerID":"mock","variant":"default"}', 1792301402252, 1792301403202, NULL, NULL, NULL, 0, NULL, NULL, NULL);
INSERT INTO session_v2 (id, project_id, workspace_id, parent_id, fork_session_id, fork_boundary, slug, directory, path, title, version, share_url, summary_additions, summary_deletions, summary_files, summary_diffs, metadata, cost, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, revert, permission, agent, model, time_created, time_updated, time_compacting, time_archived, time_suspended, resume_attempts, time_idle, time_viewed, idle_outcome) VALUES ('ses_eb27ccda4ffeKjSodHyeONASJp', 'd9af7d98dcc10e194f77fde4b8c443c5ef1ef635', NULL, NULL, NULL, NULL, 'neon-otter', '/home/user/proj', '', 'Hello from the mock model.', '1.18.32', NULL, 0, 0, 0, NULL, NULL, 0.0, 20, 10, 0, 0, 0, NULL, NULL, 'build', '{"id":"mock-model","providerID":"mock","variant":"default"}', 1792301806171, 1792301807216, NULL, NULL, NULL, 0, NULL, NULL, NULL);

CREATE TABLE "session_message" (
          `id` text PRIMARY KEY,
          `session_id` text NOT NULL,
          `type` text NOT NULL,
          `seq` integer NOT NULL,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `data` text NOT NULL,
          CONSTRAINT `fk_session_message_session_id_session_v2_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session_v2`(`id`) ON DELETE CASCADE
        );
CREATE UNIQUE INDEX `session_message_session_seq_idx` ON `session_message` (`session_id`,`seq`);
CREATE INDEX `session_message_session_time_created_id_idx` ON `session_message` (`session_id`,`time_created`,`id`);
CREATE INDEX `session_message_session_type_seq_idx` ON `session_message` (`session_id`,`type`,`seq`);
CREATE INDEX `session_message_time_created_idx` ON `session_message` (`time_created`);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_14d7d08b500137rkKb3DR11HiA', 'ses_eb282f773ffeudkJBYBQedCh1M', 'user', 0, 1792301402293, 1792301402763, '{"text":"Say hello","time":{"created":1792301402293}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_14d7d0a1b001WNmArlt4cjYWh6', 'ses_eb282f773ffeudkJBYBQedCh1M', 'assistant', 1, 1792301402651, 1792301403196, '{"agent":"build","model":{"providerID":"mock","id":"mock-model","variant":"default"},"content":[{"type":"reasoning","text":"The user wants a short greeting.","time":{"created":1792301403151,"completed":1792301403154}},{"type":"text","text":"Hello from the mock model."}],"snapshot":{"start":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","end":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e"},"finish":"stop","cost":0,"tokens":{"input":10,"output":5,"reasoning":0,"cache":{"read":0,"write":0}},"time":{"created":1792301402651,"completed":1792301403196}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_14d83328a001RWfewdJ6XwNujl', 'ses_eb27ccda4ffeKjSodHyeONASJp', 'user', 0, 1792301806218, 1792301807178, '{"text":"Please TOOL /home/user/proj/README.md","time":{"created":1792301806218}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_14d8333e6001Ek0TzWXBMYcsmG', 'ses_eb27ccda4ffeKjSodHyeONASJp', 'assistant', 1, 1792301806566, 1792301807117, '{"agent":"build","model":{"providerID":"mock","id":"mock-model","variant":"default"},"content":[{"type":"reasoning","text":"I should use a tool first.","time":{"created":1792301807047,"completed":1792301807049}},{"type":"tool","id":"call_mock_1","name":"read","state":{"status":"completed","input":{"filePath":"/home/user/proj/README.md"},"content":[{"type":"text","text":"<path>/home/user/proj/README.md</path>\n<type>file</type>\n<content>\n1: hello from the fixture\n\n(End of file - total 1 lines)\n</content>"}],"metadata":{"preview":"hello from the fixture","truncated":false,"loaded":[],"display":{"type":"file","path":"/home/user/proj/README.md","text":"hello from the fixture","lineStart":1,"lineEnd":1,"totalLines":1,"truncated":false}}},"time":{"created":1792301807057,"completed":1792301807072}}],"snapshot":{"start":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","end":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e"},"finish":"tool-calls","cost":0,"tokens":{"input":10,"output":5,"reasoning":0,"cache":{"read":0,"write":0}},"time":{"created":1792301806566,"completed":1792301807117}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_14d83360f0013b99TySrjFGXQ6', 'ses_eb27ccda4ffeKjSodHyeONASJp', 'assistant', 2, 1792301807119, 1792301807211, '{"agent":"build","model":{"providerID":"mock","id":"mock-model","variant":"default"},"content":[{"type":"text","text":"I read the file; it says hello from the fixture."}],"snapshot":{"start":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e","end":"26f38b8e93bb039e8117b54a4f6b9a015b27ba5e"},"finish":"stop","cost":0,"tokens":{"input":10,"output":5,"reasoning":0,"cache":{"read":0,"write":0}},"time":{"created":1792301807119,"completed":1792301807211}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_14d83360f001mWxXuA8qoJAfep', 'ses_eb27ccda4ffeKjSodHyeONASJp', 'system', 3, 1792301807119, 1792301807211, '{"text":"The available tools have changed.\n\nThe `read` tool now takes `path` instead of `filePath`.","time":{"created":1792301807119}}');

