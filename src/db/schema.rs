use rusqlite::Connection;

pub fn create_tables(conn: &Connection) {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tenants (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            domain TEXT,
            created_at INTEGER NOT NULL,
            UNIQUE(slug)
        );

        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            email TEXT NOT NULL,
            password_hash TEXT NOT NULL,
            email_verified INTEGER NOT NULL DEFAULT 0,
            mfa_secret TEXT,
            mfa_enabled INTEGER NOT NULL DEFAULT 0,
            force_password_change INTEGER NOT NULL DEFAULT 0,
            locked_until INTEGER,
            failed_attempts INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(tenant_id, email)
        );
        CREATE INDEX IF NOT EXISTS users_tenant_id_idx ON users(tenant_id);

        CREATE TABLE IF NOT EXISTS groups (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            description TEXT,
            UNIQUE(tenant_id, name)
        );
        CREATE INDEX IF NOT EXISTS groups_tenant_id_idx ON groups(tenant_id);

        CREATE TABLE IF NOT EXISTS user_groups (
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
            PRIMARY KEY (user_id, group_id)
        );

        CREATE TABLE IF NOT EXISTS clients (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            client_secret_hash TEXT NOT NULL,
            redirect_uris TEXT NOT NULL,
            grant_types TEXT NOT NULL,
            scopes TEXT NOT NULL,
            first_party INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS clients_tenant_id_idx ON clients(tenant_id);

        CREATE TABLE IF NOT EXISTS auth_codes (
            code TEXT PRIMARY KEY,
            client_id TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            scopes TEXT NOT NULL,
            redirect_uri TEXT NOT NULL,
            code_challenge TEXT,
            code_challenge_method TEXT,
            nonce TEXT,
            expires_at INTEGER NOT NULL,
            used INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS auth_codes_client_id_idx ON auth_codes(client_id);
        CREATE INDEX IF NOT EXISTS auth_codes_user_id_idx ON auth_codes(user_id);

        CREATE TABLE IF NOT EXISTS refresh_tokens (
            token_hash TEXT PRIMARY KEY,
            client_id TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            scopes TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            revoked INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS refresh_tokens_client_id_idx ON refresh_tokens(client_id);
        CREATE INDEX IF NOT EXISTS refresh_tokens_user_id_idx ON refresh_tokens(user_id);

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            ip TEXT NOT NULL,
            user_agent TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS sessions_user_id_idx ON sessions(user_id);

        CREATE TABLE IF NOT EXISTS backup_codes (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            code_hash TEXT NOT NULL,
            used INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS backup_codes_user_id_idx ON backup_codes(user_id);

        CREATE TABLE IF NOT EXISTS audit_log (
            id TEXT PRIMARY KEY,
            tenant_id TEXT REFERENCES tenants(id) ON DELETE SET NULL,
            user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
            event TEXT NOT NULL,
            ip TEXT,
            metadata TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS audit_log_tenant_id_idx ON audit_log(tenant_id);
        CREATE INDEX IF NOT EXISTS audit_log_user_id_idx ON audit_log(user_id);
        CREATE INDEX IF NOT EXISTS audit_log_event_idx ON audit_log(event);
        CREATE INDEX IF NOT EXISTS audit_log_created_at_idx ON audit_log(created_at);

        CREATE TABLE IF NOT EXISTS signing_keys (
            kid TEXT PRIMARY KEY,
            alg TEXT NOT NULL,
            public_key TEXT NOT NULL,
            private_key_enc TEXT NOT NULL,
            active INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS signing_keys_active_idx ON signing_keys(active);
        ",
    )
    .expect("failed to create tables");
}
