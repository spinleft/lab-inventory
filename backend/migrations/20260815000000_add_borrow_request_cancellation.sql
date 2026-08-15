BEGIN;

ALTER TABLE federation_borrow_requests
DROP CONSTRAINT IF EXISTS federation_borrow_requests_status_check;

ALTER TABLE federation_borrow_requests
ADD CONSTRAINT federation_borrow_requests_status_check
CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled'));

-- Cancelling drops the request out of `uq_federation_borrow_requests_pending_item`,
-- which is partial on `status = 'pending'`, so the item is free to be asked for
-- again without that index needing to change.

-- A federated requester reads their own requests through their guest link rather
-- than through `requester_user_id`: merging a guest link deletes the shadow
-- account it used to point at, and the requester column is ON DELETE SET NULL, so
-- the user id does not survive a merge but the link does.
CREATE INDEX idx_federation_borrow_requests_requester_link
    ON federation_borrow_requests (requester_guest_link_id, created_at DESC);

COMMIT;
