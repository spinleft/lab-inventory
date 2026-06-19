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

CREATE TABLE units (
    unit_id uuid PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    dimension TEXT NOT NULL,
    scale_to_base DOUBLE PRECISION NOT NULL,
    allow_decimal BOOLEAN NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (code <> ''),
    CHECK (name <> ''),
    CHECK (symbol <> ''),
    CHECK (dimension IN ('count', 'length', 'mass', 'volume')),
    CHECK (scale_to_base > 0)
);

INSERT INTO units (unit_id, code, name, symbol, dimension, scale_to_base, allow_decimal)
VALUES
    ('a7b2f3c4-d5e6-4f7a-8b9c-0d1e2f3a4b5c', 'pcs', 'Pieces', 'pcs', 'count', 1, false),
    ('c8d9e0f1-a2b3-4c5d-6e7f-8a9b0c1d2e3f', 'm', 'Meter', 'm', 'length', 1, true),
    ('d9eaf102-b3c4-5d6e-7f8a-9b0c1d2e3f4a', 'cm', 'Centimeter', 'cm', 'length', 0.01, true),
    ('eab02f03-c4d5-6e7f-8a9b-0c1d2e3f4a5b', 'mm', 'Millimeter', 'mm', 'length', 0.001, true),
    ('fbc03a04-d5e6-7f8a-9b0c-1d2e3f4a5b6c', 'kg', 'Kilogram', 'kg', 'mass', 1, true),
    ('0cd04b05-e6f7-8a9b-0c1d-2e3f4a5b6c7d', 'g', 'Gram', 'g', 'mass', 0.001, true),
    ('0de05c06-f708-9a0b-1c2d-3e4f5a6b7c8d', 'l', 'Liter', 'l', 'volume', 1, true),
    ('1ef06d07-a809-0b1c-2d3e-4f5a6b7c8d9e', 'ml', 'Milliliter', 'ml', 'volume', 0.001, true);

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

CREATE INDEX idx_asset_categories_path_gist
ON asset_categories USING gist(path);

CREATE TABLE locations (
    location_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    parent_location_id uuid REFERENCES locations (location_id),
    name TEXT NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (name <> ''),
    CHECK (parent_location_id IS NULL OR parent_location_id <> location_id)
);

CREATE UNIQUE INDEX idx_locations_unique_sibling_name
    ON locations (laboratory_id, COALESCE(parent_location_id, '00000000-0000-0000-0000-000000000000'::uuid), name);
CREATE INDEX idx_locations_laboratory_id ON locations (laboratory_id);
CREATE INDEX idx_locations_parent_location_id ON locations (parent_location_id);

CREATE TABLE assets (
    asset_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    category_id uuid REFERENCES asset_categories (category_id),
    asset_kind TEXT NOT NULL,
    tracking_mode TEXT NOT NULL,
    name TEXT NOT NULL,
    model TEXT,
    manufacturer TEXT,
    default_unit_id uuid NOT NULL REFERENCES units (unit_id),
    minimum_stock_quantity DOUBLE PRECISION,
    minimum_stock_unit_id uuid REFERENCES units (unit_id),
    public_notes TEXT,
    internal_notes TEXT,
    is_archived BOOLEAN NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (asset_kind IN ('equipment', 'material', 'other')),
    CHECK (tracking_mode IN ('serialized', 'quantity')),
    CHECK (minimum_stock_quantity IS NULL OR minimum_stock_quantity >= 0),
    CHECK ((minimum_stock_quantity IS NULL) = (minimum_stock_unit_id IS NULL)),
    CHECK (tracking_mode = 'quantity' OR minimum_stock_quantity IS NULL),
    CHECK (name <> '')
);

CREATE UNIQUE INDEX idx_assets_unique_laboratory_name_model
    ON assets (laboratory_id, name, COALESCE(model, ''));
CREATE INDEX idx_assets_laboratory_id ON assets (laboratory_id);
CREATE INDEX idx_assets_category_id ON assets (category_id);
CREATE INDEX idx_assets_default_unit_id ON assets (default_unit_id);
CREATE INDEX idx_assets_minimum_stock_unit_id ON assets (minimum_stock_unit_id);
CREATE INDEX idx_assets_search_trgm
    ON assets USING gin ((name || ' ' || COALESCE(model, '') || ' ' || COALESCE(manufacturer, '')) gin_trgm_ops);

CREATE TABLE asset_inventory_items (
    inventory_item_id uuid PRIMARY KEY,
    asset_id uuid NOT NULL REFERENCES assets (asset_id),
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    tracking_mode TEXT NOT NULL,
    serial_number TEXT,
    batch_number TEXT,
    quantity_on_hand DOUBLE PRECISION NOT NULL,
    quantity_allocated DOUBLE PRECISION NOT NULL DEFAULT 0,
    unit_id uuid NOT NULL REFERENCES units (unit_id),
    location_id uuid REFERENCES locations (location_id),
    status TEXT NOT NULL DEFAULT 'available',
    public_notes TEXT,
    internal_notes TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (tracking_mode IN ('serialized', 'quantity')),
    CHECK (quantity_on_hand >= 0),
    CHECK (quantity_allocated >= 0),
    CHECK (quantity_allocated <= quantity_on_hand),
    CHECK (status IN ('available', 'reserved', 'retired', 'lost', 'consumed')),
    CHECK (
        (
            tracking_mode = 'serialized'
            AND serial_number IS NOT NULL
            AND serial_number <> ''
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

CREATE UNIQUE INDEX idx_asset_inventory_items_unique_serial_number
    ON asset_inventory_items (laboratory_id, serial_number)
    WHERE serial_number IS NOT NULL;
CREATE INDEX idx_asset_inventory_items_asset_id ON asset_inventory_items (asset_id);
CREATE INDEX idx_asset_inventory_items_laboratory_id ON asset_inventory_items (laboratory_id);
CREATE INDEX idx_asset_inventory_items_location_id ON asset_inventory_items (location_id);
CREATE INDEX idx_asset_inventory_items_unit_id ON asset_inventory_items (unit_id);
CREATE INDEX idx_asset_inventory_items_search_trgm
    ON asset_inventory_items USING gin ((COALESCE(serial_number, '') || ' ' || COALESCE(batch_number, '') || ' ' || COALESCE(public_notes, '') || ' ' || COALESCE(internal_notes, '')) gin_trgm_ops);

CREATE TABLE attachments (
    attachment_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    resource_type TEXT NOT NULL,
    resource_id uuid NOT NULL,
    file_name TEXT NOT NULL,
    mime_type TEXT,
    file_size_bytes BIGINT NOT NULL,
    storage_url TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'internal',
    uploaded_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (resource_type IN ('asset', 'inventory_item')),
    CHECK (visibility IN ('public', 'internal')),
    CHECK (file_name <> ''),
    CHECK (file_size_bytes >= 0),
    CHECK (storage_url <> '')
);

CREATE INDEX idx_attachments_laboratory_id ON attachments (laboratory_id);
CREATE INDEX idx_attachments_resource ON attachments (resource_type, resource_id);
CREATE INDEX idx_attachments_visibility ON attachments (visibility);
CREATE INDEX idx_attachments_file_name_trgm
    ON attachments USING gin (file_name gin_trgm_ops);

CREATE TABLE inventory_transactions (
    transaction_id uuid PRIMARY KEY,
    inventory_item_id uuid REFERENCES asset_inventory_items (inventory_item_id) ON DELETE SET NULL,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    actor_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    actor_laboratory_id uuid REFERENCES laboratories (laboratory_id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    quantity_delta DOUBLE PRECISION NOT NULL DEFAULT 0,
    allocated_delta DOUBLE PRECISION NOT NULL DEFAULT 0,
    from_location_id uuid REFERENCES locations (location_id) ON DELETE SET NULL,
    to_location_id uuid REFERENCES locations (location_id) ON DELETE SET NULL,
    related_resource_type TEXT,
    related_resource_id uuid,
    details jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (action IN ('create', 'update', 'delete', 'adjust', 'move', 'stocktake', 'allocate', 'release_allocation')),
    CHECK (related_resource_type IS NULL)
);

CREATE INDEX idx_inventory_transactions_inventory_item_id ON inventory_transactions (inventory_item_id);
CREATE INDEX idx_inventory_transactions_laboratory_id ON inventory_transactions (laboratory_id);
CREATE INDEX idx_inventory_transactions_actor_user_id ON inventory_transactions (actor_user_id);
CREATE INDEX idx_inventory_transactions_related_resource ON inventory_transactions (related_resource_type, related_resource_id);
CREATE INDEX idx_inventory_transactions_created_at ON inventory_transactions (created_at);

COMMIT;
