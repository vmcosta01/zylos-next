//! zylos-channels — Persistência real de canais e mensagens via SQLite (sqlx).
//!
//! Substitui o backend em memória (DashMap) por um store persistente com
//! migrations individuais. Fornece `ChannelsStore` para uso no gateway.

pub mod store;
pub mod models;

pub use models::{Channel, ChannelMessage, ChannelMember};
pub use store::ChannelsStore;