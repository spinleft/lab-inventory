BEGIN;

CREATE EXTENSION IF NOT EXISTS pgcrypto;

ALTER TABLE users
ADD COLUMN is_federation_shadow BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE federation_local_nodes (
    node_id uuid PRIMARY KEY,
    public_base_url TEXT NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO federation_local_nodes (node_id, public_base_url)
VALUES (gen_random_uuid(), '');

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

COMMIT;
