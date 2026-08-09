BEGIN;

-- Allow local (same-server) guest links without a remote node
ALTER TABLE federation_guest_links
ALTER COLUMN remote_node_id DROP NOT NULL;

-- Track whether a guest link originated from federation or local cross-lab
ALTER TABLE federation_guest_links
ADD COLUMN source TEXT NOT NULL DEFAULT 'federation'
CHECK (source IN ('federation', 'local'));

-- Replace the single unique constraint with two partial indexes
-- so that federation links and local links are tracked independently.
DO $$
DECLARE
    constraint_name text;
BEGIN
    SELECT conname INTO constraint_name
    FROM pg_constraint
    WHERE conrelid = 'federation_guest_links'::regclass
      AND contype = 'u';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE federation_guest_links DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;

CREATE UNIQUE INDEX uq_federation_guest_links_federation
    ON federation_guest_links (local_laboratory_id, remote_node_id, remote_laboratory_id, remote_user_id)
    WHERE remote_node_id IS NOT NULL;

CREATE UNIQUE INDEX uq_federation_guest_links_local
    ON federation_guest_links (local_laboratory_id, remote_laboratory_id, remote_user_id)
    WHERE remote_node_id IS NULL;

COMMIT;
