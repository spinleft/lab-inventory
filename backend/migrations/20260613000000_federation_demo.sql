BEGIN;

INSERT INTO user_types (user_type_id, name, description)
VALUES ('20000000-0000-4000-8000-000000000001', 'admin', 'Local laboratory administrators')
ON CONFLICT (name) DO NOTHING;

UPDATE users
SET user_type_id = (SELECT user_type_id FROM user_types WHERE name = 'admin')
WHERE user_type_id IN (
    SELECT user_type_id
    FROM user_types
    WHERE name IN ('owner', 'maintainer')
);

UPDATE users
SET user_type_id = (SELECT user_type_id FROM user_types WHERE name = 'user')
WHERE user_type_id IN (
    SELECT user_type_id
    FROM user_types
    WHERE name = 'guest'
);

DELETE FROM user_types
WHERE name IN ('owner', 'maintainer', 'guest');

UPDATE user_types
SET description = 'Local laboratory users'
WHERE name = 'user';

INSERT INTO laboratories (laboratory_id, name, address, description)
VALUES (
    '30000000-0000-4000-8000-000000000001',
    'Local Demo Laboratory',
    'Local node',
    'Default local laboratory for federation demo deployments.'
)
ON CONFLICT (laboratory_id) DO NOTHING;

CREATE TABLE remote_laboratories (
    remote_laboratory_id uuid PRIMARY KEY,
    name TEXT NOT NULL,
    api_base_url TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    key_id TEXT NOT NULL,
    shared_secret TEXT NOT NULL,
    last_seen_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (name <> ''),
    CHECK (api_base_url <> ''),
    CHECK (key_id <> ''),
    CHECK (shared_secret <> '')
);

CREATE TABLE federation_nonces (
    remote_laboratory_id uuid NOT NULL REFERENCES remote_laboratories (remote_laboratory_id) ON DELETE CASCADE,
    nonce TEXT NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (remote_laboratory_id, nonce),
    CHECK (nonce <> '')
);

ALTER TABLE borrow_requests
    ADD COLUMN correlation_id uuid,
    ADD COLUMN direction TEXT NOT NULL DEFAULT 'local',
    ADD COLUMN remote_laboratory_id uuid REFERENCES remote_laboratories (remote_laboratory_id) ON DELETE SET NULL,
    ADD COLUMN remote_inventory_item_id uuid,
    ADD COLUMN remote_requester_user_id uuid,
    ADD COLUMN remote_requester_username TEXT,
    ADD COLUMN remote_requester_laboratory_id uuid,
    ADD COLUMN remote_requester_laboratory_name TEXT,
    ADD COLUMN remote_asset_name TEXT,
    ADD COLUMN remote_asset_model TEXT,
    ADD COLUMN remote_unit_code TEXT,
    ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'synced',
    ADD COLUMN sync_error TEXT;

ALTER TABLE borrow_requests
    ALTER COLUMN inventory_item_id DROP NOT NULL,
    ALTER COLUMN requester_user_id DROP NOT NULL,
    ALTER COLUMN requester_laboratory_id DROP NOT NULL,
    ALTER COLUMN owner_laboratory_id DROP NOT NULL;

UPDATE borrow_requests
SET correlation_id = borrow_request_id
WHERE correlation_id IS NULL;

ALTER TABLE borrow_requests
    ALTER COLUMN correlation_id SET NOT NULL,
    ADD CONSTRAINT borrow_requests_direction_check CHECK (direction IN ('local', 'requester_mirror', 'owner_authority')),
    ADD CONSTRAINT borrow_requests_sync_status_check CHECK (sync_status IN ('synced', 'sync_failed'));

CREATE UNIQUE INDEX idx_borrow_requests_correlation_direction
    ON borrow_requests (correlation_id, direction);
CREATE INDEX idx_borrow_requests_remote_laboratory_id ON borrow_requests (remote_laboratory_id);

COMMIT;
