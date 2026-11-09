# zylos-a2m

**Agent-to-Agent messaging with file transfers.**

Message bus for inter-agent communication supporting text messages, file transfers (up to 50MB), reactions, reply threading, and delivery receipts. Messages are persisted in SQLite for offline agents and delivered on reconnect.

## Features

- **Text messaging** with optional reply threading (`reply_to`)
- **File transfers** up to 50MB with SHA-256 integrity verification
- **Reactions** — emoji responses to any message
- **Delivery receipts** — `Sent`, `Delivered`, `Read` status tracking
- **Offline persistence** — messages stored in SQLite, delivered when the recipient subscribes
- **Async-first** — built on `tokio::sync::broadcast` for concurrent subscribers
- **Content-addressed storage** — transferred files deduplicated by hash

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
zylos-a2m = { path = "../zylos-a2m" }
```

### Dependencies

| Crate       | Purpose                          |
|-------------|----------------------------------|
| `tokio`     | Async runtime, broadcast channels |
| `rusqlite`  | SQLite persistence               |
| `serde`     | Message serialization            |
| `sha2`      | File integrity hashing           |
| `uuid`      | Message ID generation            |
| `chrono`    | Timestamps                       |
| `thiserror` | Error types                      |
| `hex`       | Hash encoding                    |

## Quick Start

```rust
use zylos_a2m::{A2mBus, A2mConfig, A2mMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a bus with default config (50MB max, /data/a2m-transfers)
    let config = A2mConfig::default();
    let bus = A2mBus::new(config)?;

    // Subscribe agent-b before sending
    let mut rx = bus.subscribe(&"agent-b".into()).await;

    // Send a text message
    let msg = A2mMessage::Text {
        from: "agent-a".into(),
        to: "agent-b".into(),
        body: "Hello, agent B!".into(),
        reply_to: None,
    };
    let msg_id = bus.send(msg).await?;
    println!("Sent message: {}", msg_id);

    // Receive the message
    let received = rx.recv().await?;
    println!("Received: {:?}", received);

    Ok(())
}
```

## Protocol Messages

The `A2mMessage` enum defines four message types:

### Text

```rust
A2mMessage::Text {
    from: AgentId,
    to: AgentId,
    body: String,
    reply_to: Option<MessageId>,  // set to enable threading
}
```

### File

```rust
A2mMessage::File {
    from: AgentId,
    to: AgentId,
    filename: String,
    content_hash: String,  // SHA-256 hex digest
    size: u64,
}
```

### Reaction

```rust
A2mMessage::Reaction {
    from: AgentId,
    message_id: MessageId,
    emoji: String,
}
```

### Receipt

```rust
A2mMessage::Receipt {
    message_id: MessageId,
    status: DeliveryStatus,  // Sent | Delivered | Read
}
```

### Helper methods

```rust
msg.sender()    -> Option<&AgentId>   // None for Receipt
msg.recipient() -> Option<&AgentId>   // None for Reaction/Receipt
```

## File Transfers

Files are copied to the transfer directory with a content-addressed name (SHA-256 hash). Integrity is verified on retrieval.

```rust
use std::path::Path;

// Send a file
let msg_id = bus
    .transfer_file(&"agent-a".into(), &"agent-b".into(), Path::new("/tmp/report.pdf"))
    .await?;

// Retrieve and verify integrity
let contents = bus.retrieve_file(&content_hash).await?;
// Returns Err(A2mError::IntegrityCheck) if the file was tampered with
```

### Size limit

Default maximum is **50 MB**. Configure via `A2mConfig`:

```rust
let config = A2mConfig {
    max_file_size: 10 * 1024 * 1024, // 10MB
    transfer_dir: PathBuf::from("/var/a2m-transfers"),
};
```

Exceeding the limit returns `A2mError::FileTooLarge { size, max }`.

## Reply Threading

Set `reply_to` to the parent message's ID to create a thread:

```rust
let original_id = bus.send(A2mMessage::Text {
    from: "agent-a".into(),
    to: "agent-b".into(),
    body: "Original question".into(),
    reply_to: None,
}).await?;

let reply_id = bus.send(A2mMessage::Text {
    from: "agent-b".into(),
    to: "agent-a".into(),
    body: "Answer".into(),
    reply_to: Some(original_id),
}).await?;
```

Retrieve thread history with `bus.history(&agent_id, limit)`.

## Delivery Receipts

Track message lifecycle with three statuses:

```rust
use zylos_a2m::DeliveryStatus;

// Mark as read
bus.send(A2mMessage::Receipt {
    message_id: msg_id.clone(),
    status: DeliveryStatus::Read,
}).await?;
```

| Status      | Meaning                              |
|-------------|--------------------------------------|
| `Sent`      | Message persisted in the bus         |
| `Delivered` | Recipient's subscriber received it   |
| `Read`      | Recipient acknowledged processing    |

## Offline Persistence

Messages sent to offline agents are stored in SQLite and automatically delivered when the agent calls `subscribe()`:

```rust
// Agent C is offline — message is persisted
bus.send(A2mMessage::Text {
    from: "agent-a".into(),
    to: "agent-c".into(),
    body: "You were offline".into(),
    reply_to: None,
}).await?;

// Agent C comes online — receives pending messages immediately
let mut rx = bus.subscribe(&"agent-c".into()).await;
let pending = rx.recv().await?; // "You were offline"
```

Check pending count before subscribing:

```rust
let n = bus.pending_count(&"agent-c".into()).await?;
println!("{} messages waiting", n);
```

## Bus Statistics

```rust
let stats = bus.stats().await;
println!("Total: {}, Delivered: {}, Subscribers: {}",
    stats.total_messages,
    stats.delivered_messages,
    stats.active_subscribers,
);
```

## Configuration

```rust
pub struct A2mConfig {
    /// Maximum file transfer size in bytes (default: 50MB).
    pub max_file_size: u64,
    /// Directory for storing transferred files and the SQLite database.
    pub transfer_dir: PathBuf,
}
```

The SQLite database (`a2m.db`) is created automatically inside `transfer_dir`.

## Error Handling

All public methods return `Result<T, A2mError>`:

```rust
pub enum A2mError {
    AgentNotFound(String),
    MessageNotFound(String),
    FileTooLarge { size: u64, max: u64 },
    IntegrityCheck { expected: String, actual: String },
    Io(std::io::Error),
    Database(rusqlite::Error),
    Serialization(String),
}
```

## Testing

Run the full test suite:

```bash
cargo test -p zylos-a2m
```

Tests cover:

- Text message send/receive
- Subscribe and real-time delivery
- Offline message persistence and redelivery
- Reply threading
- Reactions and receipts
- File transfer with integrity verification
- File size limit enforcement
- Integrity check failure detection
- Message history pagination
- Pending count tracking
- Bus statistics

## Architecture

```
┌──────────┐    send()     ┌─────────────┐   broadcast   ┌──────────┐
│ Agent A  │ ───────────►  │   A2mBus    │ ─────────────► │ Agent B  │
└──────────┘               │             │                └──────────┘
                           │  SQLite DB  │
┌──────────┐  subscribe()  │  (a2m.db)   │
│ Agent C  │ ◄──────────── │             │
│ (offline)│  on reconnect └─────────────┘
└──────────┘
```

- **In-memory**: `tokio::sync::broadcast` channels per subscribed agent (capacity 256)
- **On-disk**: SQLite with indexed lookups on `to_agent` and `from_agent`
- **File storage**: Content-addressed files in `transfer_dir/<sha256>`

## License

This crate is part of the Zylos project. See the [root LICENSE](../../LICENSE) file for details.