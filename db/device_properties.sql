DROP TABLE IF EXISTS device_properties;

CREATE TABLE device_properties (
    device_id      UUID        NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    property_name  TEXT        NOT NULL,
    property_value JSONB       NOT NULL,
    source         TEXT        NOT NULL,
    priority       INTEGER     NOT NULL DEFAULT 100,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, property_name)
);

COMMENT ON TABLE  device_properties                  IS 'Desired-state properties attached to a device, used as input when generating device_desired_config. A higher-priority source can override a lower one.';
COMMENT ON COLUMN device_properties.device_id        IS 'FK to devices. Cascade-deletes all property rows when the device is removed.';
COMMENT ON COLUMN device_properties.property_name    IS 'Property key, referencing a name in property_definitions. E.g. "ntp_server", "dns_primary".';
COMMENT ON COLUMN device_properties.property_value   IS 'Desired value as JSON; must conform to the data_type declared in property_definitions.';
COMMENT ON COLUMN device_properties.source           IS 'Origin of this property value, e.g. "profile", "operator", "api". Used for conflict resolution and audit.';
COMMENT ON COLUMN device_properties.priority         IS 'Lower number = higher priority. When multiple sources set the same property, the lowest priority value wins during config generation.';
COMMENT ON COLUMN device_properties.created_at       IS 'Timestamp when this property was first set.';
COMMENT ON COLUMN device_properties.updated_at       IS 'Timestamp of the most recent value change; updated by application logic.';