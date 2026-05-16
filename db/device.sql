DROP TABLE IF EXISTS device;

CREATE TABLE device (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_uid TEXT NOT NULL UNIQUE,
    manufacturer TEXT,
    oui TEXT,
    product_class TEXT,
    serial_number TEXT,
    first_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    current_protocol TEXT NOT NULL,
    software_version TEXT,
    hardware_version TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'
);