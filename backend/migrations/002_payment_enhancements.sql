-- Add receipt image (Base64 encoded)
ALTER TABLE payments ADD COLUMN receipt_image TEXT;

-- Add recurrence fields
ALTER TABLE payments ADD COLUMN is_recurring INTEGER NOT NULL DEFAULT 0;
ALTER TABLE payments ADD COLUMN recurrence_type TEXT;  -- 'daily', 'weekly', 'monthly', 'yearly'
ALTER TABLE payments ADD COLUMN recurrence_interval INTEGER DEFAULT 1;  -- every X periods
ALTER TABLE payments ADD COLUMN recurrence_times_per INTEGER;  -- NULL = every X, non-NULL = X times per period
ALTER TABLE payments ADD COLUMN recurrence_end_date TEXT;  -- NULL = no end date
