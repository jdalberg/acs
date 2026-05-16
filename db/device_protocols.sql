DROP TABLE IF EXISTS device_protocols;

CREATE TABLE device_protocols (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    protocol TEXT NOT NULL,
    endpoint_id TEXT,
    connection_request_url TEXT,
    username TEXT,
    last_session_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'
);