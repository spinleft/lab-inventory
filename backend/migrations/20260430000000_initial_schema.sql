BEGIN;

CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS ltree;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------------------
-- Laboratories and users
-- ---------------------------------------------------------------------------

CREATE TABLE laboratories (
    laboratory_id uuid PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    address TEXT NOT NULL,
    description TEXT,
    contact TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- One laboratory to start from, so a fresh install has somewhere to put the
-- default units below. Renaming it is the first thing the deployment guide
-- asks an administrator to do.
INSERT INTO laboratories (laboratory_id, name, address)
VALUES ('7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', '默认实验室', '待填写');

CREATE TABLE user_types (
    user_type_id uuid PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT
);

INSERT INTO user_types (user_type_id, name, description)
VALUES
    ('0c145f58-37ee-4778-937a-7101dfac7f45', 'root', 'Unrestricted superuser with all permissions'),
    ('be551106-757f-4518-bad3-dde0665c9e35', 'super_admin', 'Server-wide administrators with full access'),
    ('7f49552d-4f8e-42ab-8770-c02be8aeb049', 'lab_admin', 'Laboratory-scoped administrators'),
    ('7f4decd8-c017-4368-b31f-bd1427058687', 'guest', 'Read-only guest users'),
    ('7f49552d-4f8e-42ab-8770-c02be8aeb050', 'user', 'Local laboratory users');

CREATE TABLE users (
    user_id uuid PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    user_type_id uuid NOT NULL REFERENCES user_types (user_type_id),
    laboratory_id uuid REFERENCES laboratories (laboratory_id),
    email TEXT UNIQUE,
    phone_number VARCHAR(15) UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz,
    -- Set on the stand-in account a federated visitor acts through, so the
    -- account can never be treated as a local login.
    is_federation_shadow BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX idx_users_user_type_id ON users (user_type_id);
CREATE INDEX idx_users_laboratory_id ON users (laboratory_id);

-- Columns are listed rather than expanded from `users.*`: the callers of this
-- view read a fixed set, and a column added to `users` later has to be an
-- explicit decision here rather than something that leaks in.
CREATE VIEW v_users AS
SELECT
    users.user_id,
    users.username,
    users.password_hash,
    users.user_type_id,
    users.laboratory_id,
    users.email,
    users.phone_number,
    users.created_at,
    users.last_login_at,
    user_types.name AS user_type_name
FROM users
LEFT JOIN user_types ON users.user_type_id = user_types.user_type_id;

CREATE VIEW v_actors AS
SELECT users.user_id, user_types.name AS user_type_name, users.laboratory_id
FROM users
LEFT JOIN user_types ON users.user_type_id = user_types.user_type_id;

-- The password behind this hash is published in this repository, which is why
-- production configuration refuses to start until it has been changed. See
-- `src/bootstrap.rs` and docs/deployment.md.
INSERT INTO users (user_id, username, password_hash, user_type_id)
VALUES (
    'ddf8994f-d522-4659-8d02-c1d479057be6',
    'root',
    '$argon2id$v=19$m=15000,t=2,p=1$OEx/rcq+3ts//WUDzGNl2g$Am8UFBA4w5NJEmAtquGvBmAlu92q/VQcaoL5AyJPfc8',
    '0c145f58-37ee-4778-937a-7101dfac7f45'
);

CREATE TABLE audit_logs (
    audit_log_id uuid PRIMARY KEY,
    actor_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id uuid,
    details jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_logs_actor_user_id ON audit_logs (actor_user_id);
CREATE INDEX idx_audit_logs_resource ON audit_logs (resource_type, resource_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs (created_at);

CREATE TYPE header_pair AS (
    name TEXT,
    value BYTEA
);

CREATE TABLE idempotency (
   user_id uuid NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
   idempotency_key TEXT NOT NULL,
   response_status_code SMALLINT,
   response_headers header_pair[],
   response_body BYTEA,
   created_at timestamptz NOT NULL DEFAULT now(),
   PRIMARY KEY(user_id, idempotency_key)
);

-- ---------------------------------------------------------------------------
-- Categories, locations and units
-- ---------------------------------------------------------------------------

CREATE TABLE asset_categories (
    category_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories(laboratory_id),
    parent_category_id uuid REFERENCES asset_categories(category_id),
    name text NOT NULL,
    code text NOT NULL,
    path ltree NOT NULL,
    depth integer NOT NULL,
    description text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    CHECK (name <> ''),
    CHECK (code ~ '^[a-z][a-z0-9_]{0,63}$')
);

CREATE UNIQUE INDEX uq_asset_categories_sibling_name
ON asset_categories (
    laboratory_id,
    COALESCE(parent_category_id, '00000000-0000-0000-0000-000000000000'::uuid),
    name
);

CREATE UNIQUE INDEX uq_asset_categories_sibling_code
ON asset_categories (
    laboratory_id,
    COALESCE(parent_category_id, '00000000-0000-0000-0000-000000000000'::uuid),
    code
);

CREATE UNIQUE INDEX uq_asset_categories_path
ON asset_categories(laboratory_id, path);

CREATE INDEX idx_asset_categories_path_gist
ON asset_categories USING gist(path);

CREATE TABLE locations (
    location_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories(laboratory_id),
    parent_location_id uuid REFERENCES locations(location_id),
    name text NOT NULL,
    code text NOT NULL,
    path ltree NOT NULL,
    depth integer NOT NULL,
    description text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    CHECK (name <> ''),
    CHECK (code ~ '^[a-z][a-z0-9_]{0,63}$')
);

CREATE UNIQUE INDEX uq_locations_sibling_name
ON locations (
    laboratory_id,
    COALESCE(parent_location_id, '00000000-0000-0000-0000-000000000000'::uuid),
    name
);

CREATE UNIQUE INDEX uq_locations_sibling_code
ON locations (
    laboratory_id,
    COALESCE(parent_location_id, '00000000-0000-0000-0000-000000000000'::uuid),
    code
);

CREATE UNIQUE INDEX uq_locations_path
ON locations(laboratory_id, path);

-- Paired with `laboratory_id` so inventory items can carry a foreign key that
-- proves the location belongs to the same laboratory as the item.
CREATE UNIQUE INDEX uq_locations_location_laboratory
ON locations(location_id, laboratory_id);

CREATE INDEX idx_locations_path_gist
ON locations USING gist(path);

CREATE TABLE unit_dimensions (
    code text PRIMARY KEY,
    name text NOT NULL,
    description text,
    CHECK (code ~ '^[a-z][a-z0-9_]{0,63}$'),
    CHECK (name <> '')
);

INSERT INTO unit_dimensions (code, name)
VALUES
  ('count', '数量'),
  ('length', '长度'),
  ('area', '面积'),
  ('volume', '体积'),
  ('mass', '质量'),
  ('density', '密度'),
  ('time', '时间'),
  ('frequency', '频率'),
  ('temperature', '温度'),
  ('current', '电流'),
  ('voltage', '电压'),
  ('power', '功率'),
  ('energy', '能量'),
  ('luminous_intensity', '光强'),
  ('pressure', '压力'),
  ('force', '力'),
  ('torque', '扭矩');

CREATE TABLE units (
    unit_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories(laboratory_id),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    dimension TEXT NOT NULL REFERENCES unit_dimensions(code),
    scale_to_base DOUBLE PRECISION NOT NULL,
    allow_decimal BOOLEAN NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (laboratory_id, code),
    CHECK (code <> ''),
    CHECK (name <> ''),
    CHECK (symbol <> ''),
    CHECK (scale_to_base > 0)
);

-- Units are per-laboratory, so these belong to the seeded laboratory. A
-- laboratory created later starts with none and defines its own.
INSERT INTO units (unit_id, laboratory_id, code, name, symbol, dimension, scale_to_base, allow_decimal)
VALUES
  (gen_random_uuid(), '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', 'm', '米', 'm', 'length', 1, true),
  (gen_random_uuid(), '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', 'cm', '厘米', 'cm', 'length', 0.01, true),
  (gen_random_uuid(), '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', 'mm', '毫米', 'mm', 'length', 0.001, true),
  (gen_random_uuid(), '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', 'inch', '英寸', 'in', 'length', 0.0254, true),
  (gen_random_uuid(), '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', 'pcs', '件', 'pcs', 'count', 1, false);

-- ---------------------------------------------------------------------------
-- Assets and their parameters
-- ---------------------------------------------------------------------------

CREATE TABLE assets (
    asset_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    category_id uuid REFERENCES asset_categories (category_id),
    tracking_mode TEXT NOT NULL,
    name TEXT NOT NULL,
    model TEXT,
    manufacturer TEXT,
    inventory_unit_id uuid NOT NULL REFERENCES units (unit_id),
    public_notes TEXT,
    internal_notes TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (tracking_mode IN ('serialized', 'quantity')),
    CHECK (name <> '')
);

CREATE UNIQUE INDEX idx_assets_unique_laboratory_name_model
    ON assets (laboratory_id, name, COALESCE(model, ''));
CREATE UNIQUE INDEX uq_assets_asset_laboratory
    ON assets (asset_id, laboratory_id);
-- Carries `tracking_mode` so an inventory item's foreign key can pin the mode
-- to the asset's, making a mismatched item impossible to insert.
CREATE UNIQUE INDEX uq_assets_asset_laboratory_tracking_mode
    ON assets (asset_id, laboratory_id, tracking_mode);
CREATE INDEX idx_assets_laboratory_id ON assets (laboratory_id);
CREATE INDEX idx_assets_category_id ON assets (category_id);
CREATE INDEX idx_assets_inventory_unit_id ON assets (inventory_unit_id);
CREATE INDEX idx_assets_search_trgm
    ON assets USING gin ((name || ' ' || COALESCE(model, '') || ' ' || COALESCE(manufacturer, '')) gin_trgm_ops);

CREATE TYPE asset_parameter_data_type AS ENUM (
  'text',
  'number',
  'range',
  'boolean',
  'date',
  'enum'
);

CREATE TABLE asset_parameter_types (
    parameter_type_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories(laboratory_id),
    code text NOT NULL,
    name text NOT NULL,
    data_type asset_parameter_data_type NOT NULL,
    unit_dimension text REFERENCES unit_dimensions(code),
    default_unit_id uuid REFERENCES units(unit_id),
    description text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    UNIQUE (laboratory_id, code),
    UNIQUE (parameter_type_id, data_type),
    CHECK (code ~ '^[a-z][a-z0-9_]{0,63}$'),
    CHECK (name <> ''),
    CHECK (
      (data_type IN ('number', 'range'))
      OR (unit_dimension IS NULL AND default_unit_id IS NULL)
    )
);

CREATE TABLE asset_parameter_options (
    option_id uuid PRIMARY KEY,
    parameter_type_id uuid NOT NULL REFERENCES asset_parameter_types(parameter_type_id) ON DELETE CASCADE,
    code text NOT NULL,
    label text NOT NULL,
    sort_order integer NOT NULL DEFAULT 0,

    UNIQUE (parameter_type_id, code),
    UNIQUE (parameter_type_id, option_id),
    CHECK (code <> ''),
    CHECK (label <> '')
);

CREATE TABLE asset_parameter_assignments (
    assignment_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories(laboratory_id),
    parameter_type_id uuid NOT NULL REFERENCES asset_parameter_types(parameter_type_id),
    category_id uuid REFERENCES asset_categories(category_id) ON DELETE CASCADE,
    asset_id uuid REFERENCES assets(asset_id) ON DELETE CASCADE,
    default_unit_id uuid REFERENCES units(unit_id),
    applies_to_descendants boolean NOT NULL DEFAULT true,
    is_required boolean NOT NULL DEFAULT true,
    sort_order integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),

    CHECK ((category_id IS NULL) <> (asset_id IS NULL))
);

CREATE UNIQUE INDEX uq_asset_param_assignment_category
ON asset_parameter_assignments(category_id, parameter_type_id)
WHERE category_id IS NOT NULL;

CREATE UNIQUE INDEX uq_asset_param_assignment_asset
ON asset_parameter_assignments(asset_id, parameter_type_id)
WHERE asset_id IS NOT NULL;

CREATE TABLE asset_parameter_values (
    value_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories(laboratory_id),
    asset_id uuid NOT NULL REFERENCES assets(asset_id) ON DELETE CASCADE,
    parameter_type_id uuid NOT NULL,
    data_type asset_parameter_data_type NOT NULL,

    value_text text,
    value_number double precision,
    value_number_in_base double precision,
    value_range_start double precision,
    value_range_end double precision,
    value_range_start_in_base double precision,
    value_range_end_in_base double precision,
    unit_id uuid REFERENCES units(unit_id),
    value_boolean boolean,
    value_date date,
    value_option_id uuid,

    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    UNIQUE (asset_id, parameter_type_id),
    FOREIGN KEY (parameter_type_id, data_type)
        REFERENCES asset_parameter_types(parameter_type_id, data_type),
    FOREIGN KEY (parameter_type_id, value_option_id)
        REFERENCES asset_parameter_options(parameter_type_id, option_id),

    CHECK (
      (data_type = 'text' AND value_text IS NOT NULL AND value_number IS NULL AND value_number_in_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_in_base IS NULL AND value_range_end_in_base IS NULL AND unit_id IS NULL AND value_boolean IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'number' AND value_number IS NOT NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_in_base IS NULL AND value_range_end_in_base IS NULL AND value_text IS NULL AND value_boolean IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'range' AND value_range_start IS NOT NULL AND value_range_end IS NOT NULL AND value_range_start <= value_range_end AND value_text IS NULL AND value_number IS NULL AND value_number_in_base IS NULL AND value_boolean IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'boolean' AND value_boolean IS NOT NULL AND value_text IS NULL AND value_number IS NULL AND value_number_in_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_in_base IS NULL AND value_range_end_in_base IS NULL AND unit_id IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'date' AND value_date IS NOT NULL AND value_text IS NULL AND value_number IS NULL AND value_number_in_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_in_base IS NULL AND value_range_end_in_base IS NULL AND unit_id IS NULL AND value_boolean IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'enum' AND value_option_id IS NOT NULL AND value_text IS NULL AND value_number IS NULL AND value_number_in_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_in_base IS NULL AND value_range_end_in_base IS NULL AND unit_id IS NULL AND value_boolean IS NULL AND value_date IS NULL)
    ),
    CHECK (
      value_range_start_in_base IS NULL
      OR value_range_end_in_base IS NULL
      OR value_range_start_in_base <= value_range_end_in_base
    )
);

-- ---------------------------------------------------------------------------
-- Inventory
-- ---------------------------------------------------------------------------

CREATE TABLE asset_inventory_items (
    inventory_item_id uuid PRIMARY KEY,
    asset_id uuid NOT NULL,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    tracking_mode TEXT NOT NULL,
    serial_number TEXT,
    batch_number TEXT,
    quantity_on_hand NUMERIC NOT NULL,
    quantity_allocated NUMERIC NOT NULL DEFAULT 0,
    location_id uuid,
    status TEXT NOT NULL DEFAULT 'available',
    public_notes TEXT,
    internal_notes TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    last_stocktake_at timestamptz,
    FOREIGN KEY (asset_id, laboratory_id, tracking_mode)
        REFERENCES assets (asset_id, laboratory_id, tracking_mode)
        ON DELETE CASCADE,
    FOREIGN KEY (location_id, laboratory_id)
        REFERENCES locations (location_id, laboratory_id),
    CHECK (tracking_mode IN ('serialized', 'quantity')),
    CHECK (quantity_on_hand >= 0),
    CHECK (quantity_allocated >= 0),
    CHECK (quantity_allocated <= quantity_on_hand),
    CHECK (status IN ('available', 'reserved', 'borrowed', 'retired', 'lost', 'consumed')),
    CHECK (batch_number IS NULL OR btrim(batch_number) <> ''),
    CHECK (
        (
            tracking_mode = 'serialized'
            AND serial_number IS NOT NULL
            AND btrim(serial_number) <> ''
            AND quantity_on_hand = 1
            AND quantity_allocated IN (0, 1)
        )
        OR
        (
            tracking_mode = 'quantity'
            AND serial_number IS NULL
        )
    )
);

CREATE UNIQUE INDEX idx_asset_inventory_items_unique_asset_serial_number
    ON asset_inventory_items (laboratory_id, asset_id, serial_number)
    WHERE serial_number IS NOT NULL;
CREATE UNIQUE INDEX idx_asset_inventory_items_unique_quantity_aggregate
    ON asset_inventory_items (
        laboratory_id,
        asset_id,
        COALESCE(batch_number, ''),
        COALESCE(location_id, '00000000-0000-0000-0000-000000000000'::uuid),
        status
    )
    WHERE tracking_mode = 'quantity';
CREATE INDEX idx_asset_inventory_items_asset_laboratory_id ON asset_inventory_items (asset_id, laboratory_id);
CREATE INDEX idx_asset_inventory_items_laboratory_id ON asset_inventory_items (laboratory_id);
CREATE INDEX idx_asset_inventory_items_location_laboratory_id ON asset_inventory_items (location_id, laboratory_id);
CREATE INDEX idx_asset_inventory_items_laboratory_asset_id ON asset_inventory_items (laboratory_id, asset_id);
CREATE INDEX idx_asset_inventory_items_laboratory_status ON asset_inventory_items (laboratory_id, status);
CREATE INDEX idx_asset_inventory_items_laboratory_batch_number ON asset_inventory_items (laboratory_id, batch_number);
CREATE INDEX idx_asset_inventory_items_laboratory_location_id ON asset_inventory_items (laboratory_id, location_id);
CREATE INDEX idx_asset_inventory_items_search_trgm
    ON asset_inventory_items USING gin ((COALESCE(serial_number, '') || ' ' || COALESCE(batch_number, '') || ' ' || COALESCE(public_notes, '') || ' ' || COALESCE(internal_notes, '')) gin_trgm_ops);
CREATE UNIQUE INDEX uq_asset_inventory_items_item_laboratory
    ON asset_inventory_items (inventory_item_id, laboratory_id);

-- ---------------------------------------------------------------------------
-- Files and attachments
-- ---------------------------------------------------------------------------

-- A file that has been uploaded but not yet attached to anything. Unclaimed
-- rows expire, which is what stops an abandoned upload from occupying storage
-- forever.
CREATE TABLE file_uploads (
    upload_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    storage_backend TEXT NOT NULL DEFAULT 'local',
    storage_key TEXT NOT NULL UNIQUE,
    original_file_name TEXT NOT NULL,
    mime_type TEXT,
    file_size_bytes BIGINT NOT NULL,
    sha256_hex TEXT NOT NULL,
    uploaded_by_user_id uuid NOT NULL REFERENCES users (user_id),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    CHECK (storage_backend IN ('local')),
    CHECK (original_file_name <> ''),
    CHECK (file_size_bytes > 0),
    CHECK (storage_key <> ''),
    CHECK (sha256_hex ~ '^[0-9a-f]{64}$'),
    CHECK (expires_at > created_at)
);

CREATE INDEX idx_file_uploads_laboratory_active
    ON file_uploads (laboratory_id, expires_at)
    WHERE consumed_at IS NULL;
CREATE INDEX idx_file_uploads_uploaded_by_user_id
    ON file_uploads (uploaded_by_user_id);

CREATE TABLE files (
    file_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    storage_backend TEXT NOT NULL DEFAULT 'local',
    storage_key TEXT NOT NULL UNIQUE,
    original_file_name TEXT NOT NULL,
    mime_type TEXT,
    file_size_bytes BIGINT NOT NULL,
    sha256_hex TEXT NOT NULL,
    uploaded_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (storage_backend IN ('local')),
    CHECK (storage_key <> ''),
    CHECK (original_file_name <> ''),
    CHECK (file_size_bytes > 0),
    CHECK (sha256_hex ~ '^[0-9a-f]{64}$')
);

CREATE UNIQUE INDEX uq_files_file_laboratory
    ON files (file_id, laboratory_id);
CREATE INDEX idx_files_laboratory_created_at
    ON files (laboratory_id, created_at DESC);
CREATE INDEX idx_files_uploaded_by_user_id
    ON files (uploaded_by_user_id);
CREATE INDEX idx_files_sha256_hex
    ON files (sha256_hex);

CREATE TABLE asset_attachment_assignments (
    attachment_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    file_id uuid NOT NULL,
    asset_id uuid,
    inventory_item_id uuid,
    display_name TEXT NOT NULL,
    description TEXT,
    is_public boolean NOT NULL DEFAULT false,
    assigned_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (file_id, laboratory_id)
        REFERENCES files (file_id, laboratory_id),
    FOREIGN KEY (asset_id, laboratory_id)
        REFERENCES assets (asset_id, laboratory_id)
        ON DELETE CASCADE,
    FOREIGN KEY (inventory_item_id, laboratory_id)
        REFERENCES asset_inventory_items (inventory_item_id, laboratory_id)
        ON DELETE CASCADE,
    -- An attachment hangs off exactly one of the two.
    CHECK ((asset_id IS NULL) <> (inventory_item_id IS NULL)),
    CHECK (display_name <> '')
);

CREATE INDEX idx_asset_attachment_assignments_file_laboratory_id
    ON asset_attachment_assignments (file_id, laboratory_id);
CREATE INDEX idx_asset_attachment_assignments_asset_laboratory_id
    ON asset_attachment_assignments (asset_id, laboratory_id);
CREATE INDEX idx_asset_attachment_assignments_inventory_item_laboratory_id
    ON asset_attachment_assignments (inventory_item_id, laboratory_id);
CREATE INDEX idx_asset_attachment_assignments_assigned_by_user_id
    ON asset_attachment_assignments (assigned_by_user_id);
CREATE INDEX idx_asset_attachment_assignments_active_asset
    ON asset_attachment_assignments (asset_id, created_at DESC)
    WHERE asset_id IS NOT NULL;
CREATE INDEX idx_asset_attachment_assignments_active_inventory_item
    ON asset_attachment_assignments (inventory_item_id, created_at DESC)
    WHERE inventory_item_id IS NOT NULL;
CREATE INDEX idx_asset_attachment_assignments_laboratory_created_active
    ON asset_attachment_assignments (laboratory_id, created_at DESC);
CREATE INDEX idx_asset_attachment_assignments_display_name_trgm
    ON asset_attachment_assignments USING gin (display_name gin_trgm_ops);

-- ---------------------------------------------------------------------------
-- Federation
-- ---------------------------------------------------------------------------

-- This server's federation identity. Partners pin `node_id` as the primary key
-- of their own `federation_remote_nodes` row for us, so it is minted once here
-- and never changes. The `singleton` key is what keeps the table to one row.
CREATE TABLE federation_local_nodes (
    singleton BOOLEAN PRIMARY KEY DEFAULT true,
    node_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (singleton)
);

INSERT INTO federation_local_nodes (node_id)
VALUES (gen_random_uuid());

CREATE TABLE federation_remote_nodes (
    remote_node_id uuid PRIMARY KEY,
    base_url TEXT NOT NULL UNIQUE,
    display_name TEXT,
    shared_secret TEXT NOT NULL,
    shared_secret_hash TEXT NOT NULL,
    tls_certificate_sha256 TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    key_version INTEGER NOT NULL DEFAULT 1,
    last_handshake_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (base_url <> ''),
    CHECK (shared_secret <> ''),
    CHECK (shared_secret_hash <> ''),
    CHECK (status IN ('active', 'revoked')),
    CHECK (key_version > 0),
    CHECK (
        tls_certificate_sha256 IS NULL
        OR tls_certificate_sha256 ~ '^[0-9a-f]{64}$'
    )
);

-- Pairing is between nodes; access is granted per laboratory pair, which is
-- what this table records.
CREATE TABLE federation_laboratory_trusts (
    trust_id uuid PRIMARY KEY,
    local_laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id) ON DELETE CASCADE,
    remote_node_id uuid NOT NULL REFERENCES federation_remote_nodes (remote_node_id) ON DELETE CASCADE,
    remote_laboratory_id uuid NOT NULL,
    remote_laboratory_name TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz,
    CHECK (status IN ('active', 'revoked')),
    UNIQUE (local_laboratory_id, remote_node_id, remote_laboratory_id)
);

CREATE INDEX idx_federation_trusts_local_laboratory
ON federation_laboratory_trusts (local_laboratory_id, status);

CREATE INDEX idx_federation_trusts_remote
ON federation_laboratory_trusts (remote_node_id, remote_laboratory_id, status);

CREATE TABLE federation_pairing_codes (
    pairing_code_id uuid PRIMARY KEY,
    local_laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (code_hash <> ''),
    CHECK (expires_at > created_at)
);

CREATE INDEX idx_federation_pairing_codes_laboratory_active
ON federation_pairing_codes (local_laboratory_id, expires_at)
WHERE consumed_at IS NULL;

-- Spent nonces, kept until the signature they belong to would have expired
-- anyway. This is what makes a captured request unusable a second time.
CREATE TABLE federation_request_nonces (
    remote_node_id uuid NOT NULL REFERENCES federation_remote_nodes (remote_node_id) ON DELETE CASCADE,
    nonce TEXT NOT NULL,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (remote_node_id, nonce),
    CHECK (nonce <> '')
);

CREATE INDEX idx_federation_request_nonces_expires_at
ON federation_request_nonces (expires_at);

-- The local stand-in account a remote user acts through, so their activity can
-- be attributed without them having an account here.
CREATE TABLE federation_guest_links (
    link_id uuid PRIMARY KEY,
    local_laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id) ON DELETE CASCADE,
    remote_node_id uuid NOT NULL REFERENCES federation_remote_nodes (remote_node_id) ON DELETE CASCADE,
    remote_laboratory_id uuid NOT NULL,
    remote_user_id uuid NOT NULL,
    remote_username TEXT NOT NULL,
    remote_user_type TEXT NOT NULL,
    local_guest_user_id uuid NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    first_seen_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    CHECK (remote_username <> ''),
    CHECK (remote_user_type IN ('lab_admin', 'user')),
    UNIQUE (local_laboratory_id, remote_node_id, remote_laboratory_id, remote_user_id)
);

CREATE INDEX idx_federation_guest_links_laboratory
ON federation_guest_links (local_laboratory_id, last_seen_at DESC);

CREATE INDEX idx_federation_guest_links_local_guest
ON federation_guest_links (local_guest_user_id);

-- ---------------------------------------------------------------------------
-- Borrowing
-- ---------------------------------------------------------------------------

CREATE TABLE federation_borrow_requests (
    borrow_request_id uuid PRIMARY KEY,
    local_laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id) ON DELETE CASCADE,
    inventory_item_id uuid NOT NULL REFERENCES asset_inventory_items (inventory_item_id) ON DELETE CASCADE,
    requester_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    requester_username TEXT NOT NULL,
    requester_user_type TEXT NOT NULL,
    requester_guest_link_id uuid REFERENCES federation_guest_links (link_id) ON DELETE SET NULL,
    request_note TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    reviewed_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    reviewed_by_username TEXT,
    reviewed_by_user_type TEXT,
    reviewed_at timestamptz,
    decision_note TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (request_note IS NULL OR btrim(request_note) <> ''),
    CHECK (reviewed_by_username IS NULL OR btrim(reviewed_by_username) <> ''),
    CHECK (reviewed_by_user_type IS NULL OR btrim(reviewed_by_user_type) <> ''),
    CHECK (decision_note IS NULL OR btrim(decision_note) <> ''),
    CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled'))
);

-- Partial on `pending`, so cancelling or resolving a request frees the item to
-- be asked for again without this index needing to know about it.
CREATE UNIQUE INDEX uq_federation_borrow_requests_pending_item
    ON federation_borrow_requests (inventory_item_id)
    WHERE status = 'pending';

CREATE INDEX idx_federation_borrow_requests_laboratory_status
    ON federation_borrow_requests (local_laboratory_id, status, created_at DESC);

CREATE INDEX idx_federation_borrow_requests_requester_user
    ON federation_borrow_requests (requester_user_id, created_at DESC);

-- A federated requester reads their own requests through their guest link rather
-- than through `requester_user_id`: merging a guest link deletes the shadow
-- account it used to point at, and the requester column is ON DELETE SET NULL, so
-- the user id does not survive a merge but the link does.
CREATE INDEX idx_federation_borrow_requests_requester_link
    ON federation_borrow_requests (requester_guest_link_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Guest registration
-- ---------------------------------------------------------------------------

CREATE TABLE guest_registration_codes (
    registration_code_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id) ON DELETE CASCADE,
    code_hmac TEXT NOT NULL,
    created_by_user_id uuid NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    revoked_at timestamptz,
    CHECK (code_hmac ~ '^[0-9a-f]{64}$'),
    CHECK (expires_at > created_at),
    CHECK (NOT (consumed_at IS NOT NULL AND revoked_at IS NOT NULL))
);

-- At most one code outstanding per laboratory, and a code is unique while it
-- is outstanding.
CREATE UNIQUE INDEX uq_guest_registration_codes_laboratory_active
ON guest_registration_codes (laboratory_id)
WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE UNIQUE INDEX uq_guest_registration_codes_code_active
ON guest_registration_codes (code_hmac)
WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_guest_registration_codes_expires_at
ON guest_registration_codes (expires_at)
WHERE consumed_at IS NULL AND revoked_at IS NULL;

-- ---------------------------------------------------------------------------
-- Label printing
-- ---------------------------------------------------------------------------

-- Network label printers a laboratory can send QR-code labels to.
--
-- The host lives here rather than in the request body on purpose: a print
-- request names a registered printer by id, so a caller can never make the
-- server open a connection to an address of their choosing.
CREATE TABLE label_printers (
    printer_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 9100,
    model TEXT NOT NULL DEFAULT 'QL-820NWBc',
    media_kind TEXT NOT NULL,
    media_width_mm INTEGER NOT NULL,
    -- Continuous stock is cut to whatever length the label needs, so it has no
    -- fixed length; die-cut labels always do.
    media_length_mm INTEGER,
    auto_cut BOOLEAN NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (name <> ''),
    CHECK (host <> ''),
    -- Printers do not serve from privileged ports; refusing them keeps a
    -- registration from being used to probe the printer's own host.
    CHECK (port BETWEEN 1024 AND 65535),
    CHECK (media_kind IN ('continuous', 'die_cut')),
    CHECK (media_width_mm BETWEEN 1 AND 255),
    CHECK (media_length_mm IS NULL OR media_length_mm BETWEEN 1 AND 255),
    CHECK ((media_kind = 'continuous') = (media_length_mm IS NULL))
);

CREATE UNIQUE INDEX uq_label_printers_laboratory_name
ON label_printers (laboratory_id, name);

CREATE INDEX idx_label_printers_laboratory
ON label_printers (laboratory_id, name);

COMMIT;
