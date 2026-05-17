DROP TABLE IF EXISTS property_definitions;

CREATE TABLE property_definitions (
    name          TEXT        PRIMARY KEY,
    description   TEXT,
    data_type     TEXT        NOT NULL,
    default_value JSONB,
    required      BOOLEAN     NOT NULL DEFAULT false,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE  property_definitions               IS 'Global catalog of valid device property keys. Acts as a schema for the device_properties table, enabling validation and UI rendering.';
COMMENT ON COLUMN property_definitions.name          IS 'Unique property key, referenced by device_properties.property_name. Use dot-notation for namespacing, e.g. "ntp.primary_server".';
COMMENT ON COLUMN property_definitions.description   IS 'Human-readable explanation of what this property controls, displayed in the management UI.';
COMMENT ON COLUMN property_definitions.data_type     IS 'Expected JSON type of the value: "string", "number", "boolean", "array", or "object". Used for validation before writing to device_properties.';
COMMENT ON COLUMN property_definitions.default_value IS 'Fallback JSON value used during config generation when a device has no explicit device_properties row for this property. NULL means no default.';
COMMENT ON COLUMN property_definitions.required      IS 'If true, the config generator must emit an error when no value (and no default) is available for this property during desired-config generation.';
COMMENT ON COLUMN property_definitions.created_at    IS 'Timestamp when this property definition was added to the catalog.';