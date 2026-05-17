-- Platform users.
--
-- Roles come in two flavours:
--   1. is_super_admin = true  →  platform-level; can create/delete domains and
--                                promote/demote any user anywhere.
--   2. Domain-level roles are stored in domain_memberships (domain_admin,
--      domain_editor, domain_viewer) — see domain_memberships.sql.
--
-- Authentication:
--   password_hash stores a bcrypt/argon2 hash for local credentials.
--   If you integrate an external IdP (OIDC/SAML) in future, add:
--       external_provider TEXT,
--       external_id       TEXT,
--   and make password_hash nullable (already is).

DROP TABLE IF EXISTS users CASCADE;

CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        NOT NULL UNIQUE,
    display_name    TEXT,
    -- Hashed local password (bcrypt / argon2id recommended).
    -- NULL if the account is managed by an external identity provider.
    password_hash   TEXT,
    -- Global platform flag. Only super_admins can set this on other users.
    is_super_admin  BOOLEAN     NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at   TIMESTAMPTZ
);

-- Fast lookup by email during login.
CREATE INDEX idx_users_email ON users(email);

COMMENT ON TABLE  users               IS 'Platform users. Domain-level roles are in domain_memberships.';
COMMENT ON COLUMN users.is_super_admin IS 'Platform-wide admin flag. Checked without a join for performance.';
COMMENT ON COLUMN users.password_hash  IS 'bcrypt/argon2id hash. NULL for external-IdP-only accounts.';
