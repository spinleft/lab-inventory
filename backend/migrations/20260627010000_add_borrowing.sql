BEGIN;

ALTER TABLE asset_inventory_items
DROP CONSTRAINT IF EXISTS asset_inventory_items_status_check;

ALTER TABLE asset_inventory_items
ADD CONSTRAINT asset_inventory_items_status_check
CHECK (status IN ('available', 'reserved', 'borrowed', 'retired', 'lost', 'consumed'));

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
    CHECK (status IN ('pending', 'approved', 'rejected'))
);

CREATE UNIQUE INDEX uq_federation_borrow_requests_pending_item
    ON federation_borrow_requests (inventory_item_id)
    WHERE status = 'pending';

CREATE INDEX idx_federation_borrow_requests_laboratory_status
    ON federation_borrow_requests (local_laboratory_id, status, created_at DESC);

CREATE INDEX idx_federation_borrow_requests_requester_user
    ON federation_borrow_requests (requester_user_id, created_at DESC);

COMMIT;