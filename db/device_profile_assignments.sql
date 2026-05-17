DROP TABLE IF EXISTS device_profile_assignments;

CREATE TABLE device_profile_assignments (
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    profile_id UUID NOT NULL REFERENCES provisioning_profiles(id),
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, profile_id)
);