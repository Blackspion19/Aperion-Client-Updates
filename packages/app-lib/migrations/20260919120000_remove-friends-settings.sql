-- The Modrinth friends feature (and its socket/UI) has been removed from
-- this launcher; these columns only ever stored whether each section of
-- the friends sidebar list was collapsed, so they're no longer read or
-- written anywhere in the app.
ALTER TABLE settings DROP COLUMN friends_active_collapsed;

ALTER TABLE settings DROP COLUMN friends_online_collapsed;

ALTER TABLE settings DROP COLUMN friends_offline_collapsed;

ALTER TABLE settings DROP COLUMN friends_pending_collapsed;
