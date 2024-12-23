-- Drop the table and related objects
DROP TRIGGER IF EXISTS update_das_verifications_updated_at ON das_verifications;
DROP FUNCTION IF EXISTS update_updated_at_column();
DROP TABLE IF EXISTS das_verifications;
DROP TYPE IF EXISTS verification_status;
