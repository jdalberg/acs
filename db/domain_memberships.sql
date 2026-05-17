-- Domain-level role assignments.
--
-- A user can hold different roles in different domains simultaneously.
-- The three domain-scoped roles in ascending privilege order are:
--
--   domain_viewer  — read-only access to devices, events, and parameters
--                    within the domain.
--   domain_editor  — all viewer rights plus: push commands, update device
--                    config, assign provisioning profiles.
--   domain_admin   — all editor rights plus: invite/remove users from the
--                    domain, manage provisioning profiles, delete devices.
--
-- Note: super_admin (platform level) is a flag on the users table, not a role
-- here — that keeps the "is this user a platform admin?" check a simple
-- column read with no join.

DROP TABLE IF EXISTS domain_memberships CASCADE;

CREATE TABLE domain_memberships (
    user_id    UUID        NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    domain_id  UUID        NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    role       TEXT        NOT NULL
                           CHECK (role IN ('domain_admin', 'domain_editor', 'domain_viewer')),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Audit trail: which user granted this membership.
    -- SET NULL on delete so history is preserved even if the granter is removed.
    granted_by UUID        REFERENCES users(id) ON DELETE SET NULL,
    PRIMARY KEY (user_id, domain_id)
);

-- Queries like "list all members of domain X" are common in admin UIs.
CREATE INDEX idx_domain_memberships_domain_id ON domain_memberships(domain_id);

COMMENT ON TABLE  domain_memberships            IS 'Maps users to domains with a specific role. A user can have different roles in different domains.';
COMMENT ON COLUMN domain_memberships.role        IS 'domain_admin | domain_editor | domain_viewer — in descending privilege order.';
COMMENT ON COLUMN domain_memberships.granted_by  IS 'Audit: the user who assigned this membership. NULL-ed if that user is deleted.';
