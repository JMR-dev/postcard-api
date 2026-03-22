
# WebSocket API Specification v0.1

This document outlines the WebSocket protocol for the chat application. It defines the message envelope and all frame types for client-server communication.

## 0.1 — Protocol Specification

### Goal
Define the shared message envelope and all WebSocket frame types as a single source of truth that every client and the backend implement against.

### WebSocket Envelope Format
All messages sent over the WebSocket connection will be JSON objects conforming to the following structure:

```json
{
  "type": "string",
  "payload": "object",
  "id": "string",
  "timestamp": "string"
}
```

- `type`: The type of frame being sent. This determines the shape of the `payload` object.
- `payload`: A JSON object containing the data for the frame.
- `id`: A unique identifier for the message (e.g., a UUID).
- `timestamp`: An ISO 8601 formatted timestamp string indicating when the message was created.

---

### Client → Server Frame Types

#### `Authenticate`
Sent by the client to authenticate the WebSocket session upon connection.

**Payload:**
```json
{
  "token": "string" // JWT token
}
```

#### `SendMessage`
Sent by the client to send a message to a conversation.

**Payload:**
```json
{
  "conversation_id": "string",
  "client_message_id": "string", // UUID generated client-side for idempotency
  "type": "string", // "text" | "image" | "audio" | "gif" | "sticker"
  "body": "string", // Nullable
  "media_ref": "string" // Nullable, reference to uploaded media
}
```

#### `TypingStart` / `TypingStop`
Sent by the client to indicate typing activity.

**Payload:**
```json
{
  "conversation_id": "string"
}
```

#### `MarkRead`
Sent by the client to mark messages in a conversation as read up to a certain sequence number.

**Payload:**
```json
{
  "conversation_id": "string",
  "sequence": "number"
}
```

#### `AckDelivery`
Sent by the client to acknowledge the delivery of a message.

**Payload:**
```json
{
  "conversation_id": "string",
  "sequence": "number"
}
```

#### `Ping`
Sent by the client to check the connection status. The server will respond with a `Pong`.

**Payload:**
```json
{}
```

#### `Sync`
Sent by the client upon reconnection to sync missed messages.

**Payload:**
```json
{
  "conversations": [
    {
      "id": "string",
      "last_sequence": "number"
    }
  ]
}
```

---

### Server → Client Frame Types

#### `NewMessage`
Sent by the server when a new message is added to a conversation.

**Payload:**
```json
{
  "conversation_id": "string",
  "sequence": "number",
  "sender_id": "string",
  "type": "string", // "text" | "image" | "audio" | "gif" | "sticker"
  "body": "string", // Nullable
  "media": "object", // Nullable Media metadata object
  "timestamp": "string", // ISO 8601
  "client_message_id": "string" // Echoed back from sender
}
```

#### `MessageDelivered`
Sent by the server to confirm a message has been delivered.

**Payload:**
```json
{
  "conversation_id": "string",
  "sequence": "number",
  "timestamp": "string" // ISO 8601
}
```

#### `MessageRead`
Sent by the server to indicate a message has been read by a recipient.

**Payload:**
```json
{
  "conversation_id": "string",
  "sequence": "number",
  "timestamp": "string" // ISO 8601
}
```

#### `TypingIndicator`
Sent by the server to show that a user is typing in a conversation.

**Payload:**
```json
{
  "conversation_id": "string",
  "user_id": "string",
  "active": "boolean"
}
```

#### `PresenceUpdate`
Sent by the server to update a user's presence status.

**Payload:**
```json
{
  "user_id": "string",
  "status": "string", // "online" | "offline"
  "last_seen": "string" // ISO 8601
}
```

#### `MediaReady`
Sent by the server when media processing is complete and URLs are available.

**Payload:**
```json
{
  "conversation_id": "string",
  "sequence": "number",
  "media": "object" // Full Media metadata object
}
```

#### `Error`
Sent by the server when an error occurs.

**Payload:**
```json
{
  "code": "string", // See Error Code Enum
  "message": "string",
  "ref_id": "string" // Optional, correlates to the client request ID
}
```

#### `Pong`
Sent by the server in response to a client's `Ping`.

**Payload:**
```json
{}
```

---

### Media Metadata Object

This object contains metadata for media attachments.

```json
{
  "original_url": "string",
  "thumbnail_url": "string",
  "preview_url": "string",
  "width": "number",
  "height": "number",
  "duration_seconds": "number", // Nullable
  "file_size_bytes": "number",
  "mime_type": "string",
  "blurhash": "string" // Nullable
}
```

---

### Error Code Enum

- `auth_failed`: Authentication failed.
- `rate_limited`: The client is sending messages too frequently.
- `invalid_payload`: The message payload is malformed or invalid.
- `conversation_not_found`: The specified conversation does not exist.
- `internal_server_error`: An unexpected error occurred on the server.
- `not_authorized`: The client is not authorized to perform the action.

---

### Reconnection Contract

If a client disconnects, it can attempt to reconnect and sync missed messages.

1.  Client reconnects to the WebSocket.
2.  Client sends a `Sync` message.

**`Sync` Frame (Client → Server):**
```json
{
  "type": "Sync",
  "payload": {
    "conversations": [
      {
        "id": "string",
        "last_sequence": "number"
      }
    ]
  },
  "id": "string",
  "timestamp": "string"
}
```

3.  The server will respond with a batch of `NewMessage` frames for each conversation, containing all messages since the `last_sequence` number provided by the client.
