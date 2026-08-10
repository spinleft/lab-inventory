BEGIN;

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

CREATE UNIQUE INDEX uq_guest_registration_codes_laboratory_active
ON guest_registration_codes (laboratory_id)
WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE UNIQUE INDEX uq_guest_registration_codes_code_active
ON guest_registration_codes (code_hmac)
WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_guest_registration_codes_expires_at
ON guest_registration_codes (expires_at)
WHERE consumed_at IS NULL AND revoked_at IS NULL;

COMMIT;
