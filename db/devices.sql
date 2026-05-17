-- Devices observed by the ACS.
--
-- Every device belongs to exactly one domain (domain_id NOT NULL).
-- Device UIDs are unique *within* a domain; the same OUI+serial can legitimately
-- appear in two independent domains (e.g. a reseller and a direct customer both
-- managing the same hardware fleet).

DROP TABLE IF EXISTS devices CASCADE;

CREATE TABLE devices (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Tenancy: the domain that owns this device.
    domain_id        UUID        NOT NULL REFERENCES domains(id) ON DELETE RESTRICT,
    -- Composite natural key: "{oui}-{serial_number}" as sent in the Inform.
    -- Unique per domain, not globally.
    device_uid       TEXT        NOT NULL,
    manufacturer     TEXT,
    oui              TEXT,
    product_class    TEXT,
    serial_number    TEXT,
    current_protocol TEXT        NOT NULL,
    first_seen       TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen        TIMESTAMPTZ NOT NULL DEFAULT now(),
    software_version TEXT,
    hardware_version TEXT,
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    metadata         JSONB       NOT NULL DEFAULT '{}',
    UNIQUE (domain_id, device_uid)
);

-- Most queries filter by domain first.
CREATE INDEX idx_devices_domain_id ON devices(domain_id);

COMMENT ON TABLE  devices            IS 'CPE devices observed by the ACS, scoped to a domain.';
COMMENT ON COLUMN devices.domain_id  IS 'FK to domains. ON DELETE RESTRICT prevents silent domain deletion with live devices.';
COMMENT ON COLUMN devices.device_uid IS 'Composite key "{oui}-{serial_number}". Unique within a domain.';