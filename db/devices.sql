DROP TABLE IF EXISTS devices;

CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_uid TEXT NOT NULL UNIQUE,
    manufacturer TEXT,
    oui TEXT,
    product_class TEXT,
    serial_number TEXT,
    current_protocol TEXT NOT NULL,
    first_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    software_version TEXT,
    hardware_version TEXT,
    tags TEXT [] NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}'
);