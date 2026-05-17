DROP TABLE IF EXISTS device_protocols;

CREATE TABLE device_protocols (
    id                     UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id              UUID        NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    protocol               TEXT        NOT NULL,
    endpoint_id            TEXT,
    connection_request_url TEXT,
    username               TEXT,
    last_session_at        TIMESTAMPTZ,
    metadata               JSONB       NOT NULL DEFAULT '{}'
);

COMMENT ON TABLE  device_protocols                         IS 'Protocol-specific connection details for a device. A device may support multiple protocols (e.g. both CWMP and USP), one row per protocol.';
COMMENT ON COLUMN device_protocols.id                     IS 'Surrogate primary key.';
COMMENT ON COLUMN device_protocols.device_id              IS 'FK to devices. Cascade-deletes protocol records when the device is removed.';
COMMENT ON COLUMN device_protocols.protocol               IS 'Protocol identifier, e.g. "cwmp" or "usp".';
COMMENT ON COLUMN device_protocols.endpoint_id            IS 'Protocol-specific device address: the CWMP Inform endpoint or the USP endpoint ID.';
COMMENT ON COLUMN device_protocols.connection_request_url IS 'URL the ACS can use to initiate a connection request to the CPE (CWMP) or send a USP Connect record.';
COMMENT ON COLUMN device_protocols.username               IS 'Credential username used by this protocol session (CWMP connection request auth, USP agent ID, etc.).';
COMMENT ON COLUMN device_protocols.last_session_at        IS 'Timestamp of the most recent completed session over this protocol. NULL if no session has completed yet.';
COMMENT ON COLUMN device_protocols.metadata               IS 'Catch-all for protocol-specific fields that do not fit the structured columns (e.g. CWMP parameter key, USP controller cert thumbprint).';