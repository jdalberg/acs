DROP TABLE IF EXISTS device_desired_config;

CREATE TABLE device_desired_config (
    device_id UUID PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    generated_config JSONB NOT NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    config_hash TEXT NOT NULL
);