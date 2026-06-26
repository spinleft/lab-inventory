BEGIN;

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE laboratories (
    laboratory_id uuid PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    address TEXT NOT NULL,
    description TEXT,
    contact TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO laboratories (laboratory_id, name, address)
VALUES
    ('7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', '费米混合实验室', '老院区4号楼113室'),
    ('4cbd27a3-9836-4065-88f0-fdc7de22aba6', '锂镝原子实验室', '老院区4号楼110室');
    

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
    last_login_at timestamptz
);

CREATE INDEX idx_users_user_type_id ON users (user_type_id);
CREATE INDEX idx_users_laboratory_id ON users (laboratory_id);

CREATE VIEW v_users AS
SELECT users.*, user_types.name AS user_type_name
FROM users
LEFT JOIN user_types ON users.user_type_id = user_types.user_type_id;

CREATE VIEW v_actors AS
SELECT user_id, user_types.name AS user_type_name, laboratory_id
FROM users
LEFT JOIN user_types ON users.user_type_id = user_types.user_type_id;

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

CREATE EXTENSION IF NOT EXISTS ltree;

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

INSERT INTO asset_categories (category_id, laboratory_id, parent_category_id, name, code, path, depth)
VALUES
    ('8744dc59-e2ff-41e1-9ad8-95eb84ade2e0', '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', NULL, '光学', 'optical', 'optical', 0),
    ('71711a23-f348-4409-bb2a-04b67b3bbd80', '7227c5ab-78ef-43ce-87bc-5ce2337ccfe3', '8744dc59-e2ff-41e1-9ad8-95eb84ade2e0', '透镜', 'lens', 'optical.lens', 1);


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
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    dimension TEXT NOT NULL REFERENCES unit_dimensions(code),
    scale_to_base DOUBLE PRECISION NOT NULL,
    allow_decimal BOOLEAN NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (code <> ''),
    CHECK (name <> ''),
    CHECK (symbol <> ''),
    CHECK (scale_to_base > 0)
);

INSERT INTO units (unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal)
VALUES
  (gen_random_uuid(), 'm', '米', 'm', 'length', 1, true),
  (gen_random_uuid(), 'cm', '厘米', 'cm', 'length', 0.01, true),
  (gen_random_uuid(), 'mm', '毫米', 'mm', 'length', 0.001, true),
  (gen_random_uuid(), 'inch', '英寸', 'in', 'length', 0.0254, true),
  (gen_random_uuid(), 'pcs', '件', 'pcs', 'count', 1, false);

CREATE TABLE assets (
    asset_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    category_id uuid REFERENCES asset_categories (category_id),
    tracking_mode TEXT NOT NULL,
    name TEXT NOT NULL,
    model TEXT,
    manufacturer TEXT,
    default_unit_id uuid NOT NULL REFERENCES units (unit_id),
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
CREATE UNIQUE INDEX uq_assets_asset_laboratory_tracking_mode
    ON assets (asset_id, laboratory_id, tracking_mode);
CREATE INDEX idx_assets_laboratory_id ON assets (laboratory_id);
CREATE INDEX idx_assets_category_id ON assets (category_id);
CREATE INDEX idx_assets_default_unit_id ON assets (default_unit_id);
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
    value_number_base double precision,
    value_range_start double precision,
    value_range_end double precision,
    value_range_start_base double precision,
    value_range_end_base double precision,
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
      (data_type = 'text' AND value_text IS NOT NULL AND value_number IS NULL AND value_number_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_base IS NULL AND value_range_end_base IS NULL AND unit_id IS NULL AND value_boolean IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'number' AND value_number IS NOT NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_base IS NULL AND value_range_end_base IS NULL AND value_text IS NULL AND value_boolean IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'range' AND value_range_start IS NOT NULL AND value_range_end IS NOT NULL AND value_range_start <= value_range_end AND value_text IS NULL AND value_number IS NULL AND value_number_base IS NULL AND value_boolean IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'boolean' AND value_boolean IS NOT NULL AND value_text IS NULL AND value_number IS NULL AND value_number_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_base IS NULL AND value_range_end_base IS NULL AND unit_id IS NULL AND value_date IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'date' AND value_date IS NOT NULL AND value_text IS NULL AND value_number IS NULL AND value_number_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_base IS NULL AND value_range_end_base IS NULL AND unit_id IS NULL AND value_boolean IS NULL AND value_option_id IS NULL)
      OR
      (data_type = 'enum' AND value_option_id IS NOT NULL AND value_text IS NULL AND value_number IS NULL AND value_number_base IS NULL AND value_range_start IS NULL AND value_range_end IS NULL AND value_range_start_base IS NULL AND value_range_end_base IS NULL AND unit_id IS NULL AND value_boolean IS NULL AND value_date IS NULL)
    ),
    CHECK (
      value_range_start_base IS NULL
      OR value_range_end_base IS NULL
      OR value_range_start_base <= value_range_end_base
    )
);

CREATE TABLE asset_inventory_items (
    inventory_item_id uuid PRIMARY KEY,
    asset_id uuid NOT NULL,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    tracking_mode TEXT NOT NULL,
    serial_number TEXT,
    batch_number TEXT,
    quantity_on_hand NUMERIC NOT NULL,
    quantity_allocated NUMERIC NOT NULL DEFAULT 0,
    quantity_unit_id uuid NOT NULL REFERENCES units (unit_id),
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
    CHECK (status IN ('available', 'reserved', 'retired', 'lost', 'consumed')),
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
        status,
        quantity_unit_id
    )
    WHERE tracking_mode = 'quantity';
CREATE INDEX idx_asset_inventory_items_asset_laboratory_id ON asset_inventory_items (asset_id, laboratory_id);
CREATE INDEX idx_asset_inventory_items_laboratory_id ON asset_inventory_items (laboratory_id);
CREATE INDEX idx_asset_inventory_items_location_laboratory_id ON asset_inventory_items (location_id, laboratory_id);
CREATE INDEX idx_asset_inventory_items_quantity_unit_id ON asset_inventory_items (quantity_unit_id);
CREATE INDEX idx_asset_inventory_items_laboratory_asset_id ON asset_inventory_items (laboratory_id, asset_id);
CREATE INDEX idx_asset_inventory_items_laboratory_status ON asset_inventory_items (laboratory_id, status);
CREATE INDEX idx_asset_inventory_items_laboratory_batch_number ON asset_inventory_items (laboratory_id, batch_number);
CREATE INDEX idx_asset_inventory_items_laboratory_location_id ON asset_inventory_items (laboratory_id, location_id);
CREATE INDEX idx_asset_inventory_items_search_trgm
    ON asset_inventory_items USING gin ((COALESCE(serial_number, '') || ' ' || COALESCE(batch_number, '') || ' ' || COALESCE(public_notes, '') || ' ' || COALESCE(internal_notes, '')) gin_trgm_ops);
CREATE UNIQUE INDEX uq_asset_inventory_items_item_laboratory
    ON asset_inventory_items (inventory_item_id, laboratory_id);

CREATE TABLE attachment_uploads (
    upload_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    storage_backend TEXT NOT NULL DEFAULT 'local',
    storage_key TEXT NOT NULL UNIQUE,
    original_file_name TEXT NOT NULL,
    mime_type TEXT,
    file_size_bytes BIGINT NOT NULL,
    sha256_hex TEXT NOT NULL,
    uploaded_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
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

CREATE INDEX idx_attachment_uploads_laboratory_active
    ON attachment_uploads (laboratory_id, expires_at)
    WHERE consumed_at IS NULL;
CREATE INDEX idx_attachment_uploads_uploaded_by_user_id
    ON attachment_uploads (uploaded_by_user_id);

CREATE TABLE attachments (
    attachment_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    asset_id uuid,
    inventory_item_id uuid,
    display_name TEXT NOT NULL,
    original_file_name TEXT NOT NULL,
    description TEXT,
    mime_type TEXT,
    file_size_bytes BIGINT NOT NULL,
    sha256_hex TEXT NOT NULL,
    storage_backend TEXT NOT NULL DEFAULT 'local',
    storage_key TEXT NOT NULL UNIQUE,
    visibility TEXT NOT NULL DEFAULT 'internal',
    uploaded_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    deleted_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    FOREIGN KEY (asset_id, laboratory_id)
        REFERENCES assets (asset_id, laboratory_id)
        ON DELETE CASCADE,
    FOREIGN KEY (inventory_item_id, laboratory_id)
        REFERENCES asset_inventory_items (inventory_item_id, laboratory_id)
        ON DELETE CASCADE,
    CHECK ((asset_id IS NULL) <> (inventory_item_id IS NULL)),
    CHECK (visibility IN ('public', 'internal')),
    CHECK (storage_backend IN ('local')),
    CHECK (display_name <> ''),
    CHECK (original_file_name <> ''),
    CHECK (file_size_bytes > 0),
    CHECK (storage_key <> ''),
    CHECK (sha256_hex ~ '^[0-9a-f]{64}$')
);

CREATE INDEX idx_attachments_asset_laboratory_id ON attachments (asset_id, laboratory_id);
CREATE INDEX idx_attachments_inventory_item_laboratory_id ON attachments (inventory_item_id, laboratory_id);
CREATE INDEX idx_attachments_uploaded_by_user_id ON attachments (uploaded_by_user_id);
CREATE INDEX idx_attachments_deleted_by_user_id ON attachments (deleted_by_user_id);
CREATE INDEX idx_attachments_active_asset
    ON attachments (asset_id, created_at DESC)
    WHERE deleted_at IS NULL AND asset_id IS NOT NULL;
CREATE INDEX idx_attachments_active_inventory_item
    ON attachments (inventory_item_id, created_at DESC)
    WHERE deleted_at IS NULL AND inventory_item_id IS NOT NULL;
CREATE INDEX idx_attachments_laboratory_created_active
    ON attachments (laboratory_id, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_attachments_visibility_active
    ON attachments (visibility)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_attachments_display_name_trgm
    ON attachments USING gin (display_name gin_trgm_ops);

COMMIT;
