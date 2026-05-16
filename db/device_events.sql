DROP TABLE IF EXISTS device_events;

CREATE TABLE device_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id UUID REFERENCES devices(id) ON DELETE
    SET
        NULL,
        event_type TEXT NOT NULL,
        protocol TEXT NOT NULL,
        received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
        payload JSONB NOT NULL
);

CREATE INDEX idx_device_events_device_id ON device_events(device_id);

CREATE INDEX idx_device_events_received_at ON device_events(received_at DESC);

CREATE INDEX idx_device_events_payload_gin ON device_events USING GIN(payload);