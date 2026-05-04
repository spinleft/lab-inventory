BEGIN;

ALTER TABLE user_groups RENAME TO user_types;
ALTER TABLE user_types RENAME COLUMN group_id TO user_type_id;
ALTER TABLE users RENAME COLUMN group_id TO user_type_id;

ALTER TABLE user_types RENAME CONSTRAINT user_groups_pkey TO user_types_pkey;
ALTER TABLE user_types RENAME CONSTRAINT user_groups_name_key TO user_types_name_key;
ALTER TABLE users RENAME CONSTRAINT users_group_id_fkey TO users_user_type_id_fkey;
ALTER INDEX idx_users_group_id RENAME TO idx_users_user_type_id;

UPDATE user_types
SET
    name = 'owner',
    description = 'System owners'
WHERE name = 'system_admin';

UPDATE user_types
SET
    name = 'maintainer',
    description = 'Laboratory maintainers'
WHERE name = 'lab_admin';

COMMIT;
