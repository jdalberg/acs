DROP TABLE IF EXISTS device_events;

CREATE TABLE device_events (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id   UUID        REFERENCES devices(id) ON DELETE SET NULL,
    event_type  TEXT        NOT NULL,
    protocol    TEXT        NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    payload     JSONB       NOT NULL
);

CREATE INDEX idx_device_events_device_id  ON device_events(device_id);
CREATE INDEX idx_device_events_received_at ON device_events(received_at DESC);
CREATE INDEX idx_device_events_payload_gin ON device_events USING GIN(payload);

COMMENT ON TABLE  device_events             IS 'Immutable audit log of all events received from CPE devices (Inform, value-change notifications, transfer completions, etc.).';
COMMENT ON COLUMN device_events.id          IS 'Surrogate primary key.';
COMMENT ON COLUMN device_events.device_id   IS 'FK to devices. SET NULL on device deletion so the event history is retained for auditing even after a device is removed.';
COMMENT ON COLUMN device_events.event_type  IS 'Logical event classification, e.g. "inform", "value_change", "transfer_complete", "command_response".';
COMMENT ON COLUMN device_events.protocol    IS 'Protocol that delivered this event, e.g. "cwmp" or "usp".';
COMMENT ON COLUMN device_events.received_at IS 'Wall-clock time when the ACS received this event (not the CPE timestamp).';
COMMENT ON COLUMN device_events.payload     IS 'Full event data as JSON; schema varies by event_type. GIN-indexed for flexible ad-hoc queries.';