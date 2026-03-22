-- Atomic sequence increment for message ordering
CREATE OR REPLACE FUNCTION next_message_sequence(conv_id UUID)
RETURNS BIGINT AS $$
DECLARE
  seq BIGINT;
BEGIN
  UPDATE conversations
  SET next_sequence = next_sequence + 1, updated_at = now()
  WHERE id = conv_id
  RETURNING next_sequence INTO seq;
  RETURN seq;
END;
$$ LANGUAGE plpgsql;
