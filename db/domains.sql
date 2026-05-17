-- Domains are the top-level tenancy unit.
--
-- Every device, provisioning profile, and domain membership belongs to a domain.
-- A super_admin (see users.sql) can create and delete domains.
-- Domain-level access is controlled via domain_memberships.sql.

DROP TABLE IF EXISTS domains CASCADE;

CREATE TABLE domains (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL UNIQUE,
    -- URL-safe short name used in API paths and NATS subjects.
    -- Convention: lowercase letters, digits, hyphens only.
    slug        TEXT        NOT NULL UNIQUE
                            CHECK (slug ~ '^[a-z0-9][a-z0-9\-]*[a-z0-9]$'),
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE  domains            IS 'Top-level tenancy unit; every device and profile is scoped to a domain.';
COMMENT ON COLUMN domains.slug       IS 'URL-safe identifier (lowercase, hyphens). Used in NATS subjects and API routes.';
COMMENT ON COLUMN domains.updated_at IS 'Updated by application logic on any column change.';
