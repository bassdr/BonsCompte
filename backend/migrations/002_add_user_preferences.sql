-- Add user preference columns for internationalization
ALTER TABLE users ADD COLUMN language TEXT NOT NULL DEFAULT 'en';
ALTER TABLE users ADD COLUMN date_format TEXT NOT NULL DEFAULT 'yyyy-MM-dd';
ALTER TABLE users ADD COLUMN currency_position TEXT NOT NULL DEFAULT 'before';
ALTER TABLE users ADD COLUMN decimal_separator TEXT NOT NULL DEFAULT '.';

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_users_language ON users(language);
