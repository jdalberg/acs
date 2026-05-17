DROP TABLE IF EXISTS property_definitions;

CREATE TABLE property_definitions (
    name TEXT PRIMARY KEY,
    description TEXT,
    data_type TEXT NOT NULL,
    default_value JSONB,
    required BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);