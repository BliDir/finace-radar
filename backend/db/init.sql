CREATE TABLE IF NOT EXISTS emails (
    id BIGSERIAL PRIMARY KEY,
    gmail_message_id TEXT NOT NULL UNIQUE,
    month TEXT NOT NULL,
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    received_date DATE,
    snippet TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS email_analyses (
    id BIGSERIAL PRIMARY KEY,
    email_id BIGINT NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    is_finance BOOLEAN NOT NULL,
    direction TEXT NOT NULL,
    amount DOUBLE PRECISION,
    currency TEXT,
    from_party TEXT,
    to_party TEXT,
    account TEXT,
    account_type TEXT,
    merchant TEXT,
    category TEXT,
    confidence TEXT NOT NULL,
    raw_result JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS transactions (
    id BIGSERIAL PRIMARY KEY,
    email_analysis_id BIGINT NOT NULL REFERENCES email_analyses(id) ON DELETE CASCADE,
    transaction_date DATE,
    direction TEXT NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    currency TEXT NOT NULL,
    from_party TEXT,
    to_party TEXT,
    account TEXT,
    merchant TEXT,
    category TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS emails_month_idx ON emails(month);
CREATE INDEX IF NOT EXISTS email_analyses_email_id_idx ON email_analyses(email_id);
CREATE INDEX IF NOT EXISTS transactions_transaction_date_idx ON transactions(transaction_date);
CREATE INDEX IF NOT EXISTS transactions_account_idx ON transactions(account);
CREATE INDEX IF NOT EXISTS transactions_category_idx ON transactions(category);
