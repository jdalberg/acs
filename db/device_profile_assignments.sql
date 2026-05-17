DROP TABLE IF EXISTS device_profile_assignments;

CREATE TABLE device_profile_assignments (
    device_id   UUID        NOT NULL REFERENCES devices(id)              ON DELETE CASCADE,
    profile_id  UUID        NOT NULL REFERENCES provisioning_profiles(id) ON DELETE RESTRICT,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, profile_id)
);

COMMENT ON TABLE  device_profile_assignments             IS 'Many-to-many: which provisioning profiles are assigned to which devices. A device may have multiple active profiles.';
COMMENT ON COLUMN device_profile_assignments.device_id   IS 'FK to devices. Cascade-deletes assignments when the device is removed.';
COMMENT ON COLUMN device_profile_assignments.profile_id  IS 'FK to provisioning_profiles. RESTRICT prevents deletion of a profile that is still actively assigned to devices.';
COMMENT ON COLUMN device_profile_assignments.assigned_at IS 'Timestamp when this profile was assigned to the device; useful for ordering and audit.';