-- Provisioning profiles scoped to a domain (or shared across all domains).
--
-- domain_id = NULL  →  "system / shared" profile, visible to all domains.
--                       Only super_admins can create or modify shared profiles.
-- domain_id = <id>  →  profile is private to that domain; domain_admins manage it.
--
-- Profile names are unique within the same domain scope (including shared).

DROP TABLE IF EXISTS provisioning_profiles CASCADE;

CREATE TABLE provisioning_profiles (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- NULL means this is a shared/system-wide template.
    domain_id   UUID        REFERENCES domains(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    description TEXT,
    config      JSONB       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Name must be unique within the same domain scope.
    -- NULLS NOT DISTINCT ensures two shared (NULL domain_id) profiles can't
    -- share the same name (requires PostgreSQL 15+).
    UNIQUE NULLS NOT DISTINCT (domain_id, name)
);

-- Frequently queried to list profiles available to a domain.
CREATE INDEX idx_provisioning_profiles_domain_id ON provisioning_profiles(domain_id);

COMMENT ON TABLE  provisioning_profiles           IS 'Provisioning config profiles. NULL domain_id = shared/system template.';
COMMENT ON COLUMN provisioning_profiles.domain_id IS 'NULL = shared across all domains (super_admin managed). Non-null = domain-private.';