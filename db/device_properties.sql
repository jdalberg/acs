DROP TABLE IF EXISTS device_properties;

CREATE TABLE device_properties (
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    property_name TEXT NOT NULL,
    property_value JSONB NOT NULL,
    source TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, property_name)
);