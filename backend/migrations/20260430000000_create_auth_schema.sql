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

CREATE TABLE user_groups (
    group_id uuid PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT
);

INSERT INTO user_groups (group_id, name, description)
VALUES
    ('00000000-0000-0000-0000-000000000001', 'system_admin', 'System administrators'),
    ('00000000-0000-0000-0000-000000000002', 'lab_admin', 'Laboratory administrators'),
    ('00000000-0000-0000-0000-000000000003', 'user', 'Regular laboratory users'),
    ('00000000-0000-0000-0000-000000000004', 'guest', 'Read-only guest users');

CREATE TABLE users (
    user_id uuid PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    group_id uuid NOT NULL REFERENCES user_groups (group_id),
    laboratory_id uuid REFERENCES laboratories (laboratory_id),
    email TEXT UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz
);

CREATE INDEX idx_users_group_id ON users (group_id);
CREATE INDEX idx_users_laboratory_id ON users (laboratory_id);

INSERT INTO users (user_id, username, password_hash, group_id, laboratory_id, email)
VALUES (
    'ddf8994f-d522-4659-8d02-c1d479057be6',
    'admin',
    '$argon2id$v=19$m=15000,t=2,p=1$OEx/rcq+3ts//WUDzGNl2g$Am8UFBA4w5NJEmAtquGvBmAlu92q/VQcaoL5AyJPfc8',
    '00000000-0000-0000-0000-000000000001',
    NULL,
    NULL
);

CREATE TABLE audit_logs (
    audit_log_id uuid PRIMARY KEY,
    actor_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    actor_laboratory_id uuid REFERENCES laboratories (laboratory_id) ON DELETE SET NULL,
    target_laboratory_id uuid REFERENCES laboratories (laboratory_id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id uuid,
    details jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_logs_actor_user_id ON audit_logs (actor_user_id);
CREATE INDEX idx_audit_logs_actor_laboratory_id ON audit_logs (actor_laboratory_id);
CREATE INDEX idx_audit_logs_target_laboratory_id ON audit_logs (target_laboratory_id);
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
    ('10000000-0000-0000-0000-000000000001', 'pcs', 'Pieces', 'pcs', 'count', 1, false),
    ('10000000-0000-0000-0000-000000000002', 'm', 'Meter', 'm', 'length', 1, true),
    ('10000000-0000-0000-0000-000000000003', 'cm', 'Centimeter', 'cm', 'length', 0.01, true),
    ('10000000-0000-0000-0000-000000000004', 'mm', 'Millimeter', 'mm', 'length', 0.001, true),
    ('10000000-0000-0000-0000-000000000005', 'kg', 'Kilogram', 'kg', 'mass', 1, true),
    ('10000000-0000-0000-0000-000000000006', 'g', 'Gram', 'g', 'mass', 0.001, true),
    ('10000000-0000-0000-0000-000000000007', 'l', 'Liter', 'l', 'volume', 1, true),
    ('10000000-0000-0000-0000-000000000008', 'ml', 'Milliliter', 'ml', 'volume', 0.001, true);

CREATE TABLE asset_categories (
    category_id uuid PRIMARY KEY,
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    name TEXT NOT NULL,
    description TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (laboratory_id, name),
    CHECK (name <> '')
);

CREATE INDEX idx_asset_categories_laboratory_id ON asset_categories (laboratory_id);

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
    is_cross_lab_borrowable BOOLEAN NOT NULL DEFAULT false,
    public_notes TEXT,
    internal_notes TEXT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (tracking_mode IN ('serialized', 'quantity')),
    CHECK (quantity_on_hand >= 0),
    CHECK (quantity_allocated >= 0),
    CHECK (quantity_allocated <= quantity_on_hand),
    CHECK (status IN ('available', 'reserved', 'borrowed', 'maintenance', 'retired', 'lost', 'consumed')),
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

CREATE TABLE borrow_requests (
    borrow_request_id uuid PRIMARY KEY,
    inventory_item_id uuid NOT NULL REFERENCES asset_inventory_items (inventory_item_id),
    requester_user_id uuid NOT NULL REFERENCES users (user_id),
    requester_laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    owner_laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    requested_quantity DOUBLE PRECISION NOT NULL,
    unit_id uuid NOT NULL REFERENCES units (unit_id),
    expected_borrowed_at timestamptz,
    expected_returned_at timestamptz,
    purpose TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reviewed_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    reviewed_at timestamptz,
    review_comment TEXT,
    borrowed_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    borrowed_at timestamptz,
    returned_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    returned_at timestamptz,
    cancelled_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    cancelled_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (requester_laboratory_id <> owner_laboratory_id),
    CHECK (requested_quantity > 0),
    CHECK (purpose <> ''),
    CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled', 'borrowed', 'returned', 'overdue')),
    CHECK (expected_returned_at IS NULL OR expected_borrowed_at IS NULL OR expected_returned_at > expected_borrowed_at)
);

CREATE INDEX idx_borrow_requests_inventory_item_id ON borrow_requests (inventory_item_id);
CREATE INDEX idx_borrow_requests_requester_user_id ON borrow_requests (requester_user_id);
CREATE INDEX idx_borrow_requests_requester_laboratory_id ON borrow_requests (requester_laboratory_id);
CREATE INDEX idx_borrow_requests_owner_laboratory_id ON borrow_requests (owner_laboratory_id);
CREATE INDEX idx_borrow_requests_status ON borrow_requests (status);
CREATE INDEX idx_borrow_requests_created_at ON borrow_requests (created_at);
CREATE INDEX idx_borrow_requests_purpose_trgm
    ON borrow_requests USING gin (purpose gin_trgm_ops);

CREATE TABLE maintenance_records (
    maintenance_record_id uuid PRIMARY KEY,
    asset_id uuid REFERENCES assets (asset_id),
    inventory_item_id uuid REFERENCES asset_inventory_items (inventory_item_id),
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    maintenance_type TEXT NOT NULL,
    maintained_at timestamptz NOT NULL,
    responsible_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    description TEXT NOT NULL,
    public_notes TEXT,
    internal_notes TEXT,
    created_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((asset_id IS NULL) <> (inventory_item_id IS NULL)),
    CHECK (maintenance_type <> ''),
    CHECK (description <> '')
);

CREATE INDEX idx_maintenance_records_asset_id ON maintenance_records (asset_id);
CREATE INDEX idx_maintenance_records_inventory_item_id ON maintenance_records (inventory_item_id);
CREATE INDEX idx_maintenance_records_laboratory_id ON maintenance_records (laboratory_id);
CREATE INDEX idx_maintenance_records_maintained_at ON maintenance_records (maintained_at);
CREATE INDEX idx_maintenance_records_search_trgm
    ON maintenance_records USING gin ((maintenance_type || ' ' || description || ' ' || COALESCE(public_notes, '') || ' ' || COALESCE(internal_notes, '')) gin_trgm_ops);

CREATE TABLE maintenance_schedules (
    maintenance_schedule_id uuid PRIMARY KEY,
    asset_id uuid REFERENCES assets (asset_id),
    inventory_item_id uuid REFERENCES asset_inventory_items (inventory_item_id),
    laboratory_id uuid NOT NULL REFERENCES laboratories (laboratory_id),
    schedule_name TEXT NOT NULL,
    interval_days INTEGER NOT NULL,
    next_maintenance_at timestamptz NOT NULL,
    remind_before_days INTEGER NOT NULL DEFAULT 7,
    is_active BOOLEAN NOT NULL DEFAULT true,
    public_notes TEXT,
    internal_notes TEXT,
    created_by_user_id uuid REFERENCES users (user_id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((asset_id IS NULL) <> (inventory_item_id IS NULL)),
    CHECK (schedule_name <> ''),
    CHECK (interval_days > 0),
    CHECK (remind_before_days >= 0)
);

CREATE INDEX idx_maintenance_schedules_asset_id ON maintenance_schedules (asset_id);
CREATE INDEX idx_maintenance_schedules_inventory_item_id ON maintenance_schedules (inventory_item_id);
CREATE INDEX idx_maintenance_schedules_laboratory_id ON maintenance_schedules (laboratory_id);
CREATE INDEX idx_maintenance_schedules_next_maintenance_at ON maintenance_schedules (next_maintenance_at);
CREATE INDEX idx_maintenance_schedules_name_trgm
    ON maintenance_schedules USING gin (schedule_name gin_trgm_ops);

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
    CHECK (resource_type IN ('asset', 'inventory_item', 'maintenance_record', 'borrow_request')),
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
    CHECK (action IN ('create', 'update', 'delete', 'adjust', 'move', 'stocktake', 'allocate', 'release_allocation', 'borrow_out', 'return')),
    CHECK (related_resource_type IS NULL OR related_resource_type IN ('borrow_request'))
);

CREATE INDEX idx_inventory_transactions_inventory_item_id ON inventory_transactions (inventory_item_id);
CREATE INDEX idx_inventory_transactions_laboratory_id ON inventory_transactions (laboratory_id);
CREATE INDEX idx_inventory_transactions_actor_user_id ON inventory_transactions (actor_user_id);
CREATE INDEX idx_inventory_transactions_related_resource ON inventory_transactions (related_resource_type, related_resource_id);
CREATE INDEX idx_inventory_transactions_created_at ON inventory_transactions (created_at);

COMMIT;
