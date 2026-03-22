-- Unique constraints on (conversation_id, sequence) and (client_msg_id) 
-- inherently create indexes in PostgreSQL, so we only need to explicitly 
-- define the remaining requested indexes.

CREATE INDEX idx_messages_conv_created_at ON messages(conversation_id, created_at);
CREATE INDEX idx_conv_participants_user_id ON conversation_participants(user_id);
CREATE INDEX idx_stickers_pack_id ON stickers(pack_id);
