DROP TABLE IF EXISTS device_parameters;

CREATE TABLE device_parameters (
    device_id       UUID        NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    parameter_name  TEXT        NOT NULL,
    parameter_value TEXT,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, parameter_name)
);

COMMENT ON TABLE  device_parameters                  IS 'Last-known parameter values reported by a device (observed reality). One row per parameter path per device.';
COMMENT ON COLUMN device_parameters.device_id        IS 'FK to devices. Cascade-deletes all parameter rows when the device is removed.';
COMMENT ON COLUMN device_parameters.parameter_name   IS 'Full TR-069/USP parameter path, e.g. "Device.DeviceInfo.SoftwareVersion".';
COMMENT ON COLUMN device_parameters.parameter_value  IS 'String representation of the parameter value as last reported by the CPE. NULL if the value was explicitly empty or not yet received.';
COMMENT ON COLUMN device_parameters.updated_at       IS 'Timestamp of the most recent update; set by the controller when a GetParameterValues response or value-change notification is processed.';