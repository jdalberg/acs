DROP TABLE IF EXISTS device_desired_config;

CREATE TABLE device_desired_config (
    device_id        UUID        PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    generated_config JSONB       NOT NULL,
    generated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    config_hash      TEXT        NOT NULL
);

COMMENT ON TABLE  device_desired_config                  IS 'Latest desired-state config snapshot generated for a device, derived from its assigned profiles and properties.';
COMMENT ON COLUMN device_desired_config.device_id        IS 'FK to devices. 1-to-1: each device has at most one active desired config record.';
COMMENT ON COLUMN device_desired_config.generated_config IS 'Full desired config as a JSON object; structure is protocol/profile-dependent.';
COMMENT ON COLUMN device_desired_config.generated_at     IS 'Timestamp when this config snapshot was computed.';
COMMENT ON COLUMN device_desired_config.config_hash      IS 'Stable hash (e.g. SHA-256) of generated_config, used to detect drift without diffing the full JSON.';