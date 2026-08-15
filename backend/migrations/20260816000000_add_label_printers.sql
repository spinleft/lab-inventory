BEGIN;

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
