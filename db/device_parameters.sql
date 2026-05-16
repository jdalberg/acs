DROP TABLE IF EXISTS device_parameters;

CREATE TABLE device_parameters (
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    parameter_name TEXT NOT NULL,
    parameter_value TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, parameter_name)
);