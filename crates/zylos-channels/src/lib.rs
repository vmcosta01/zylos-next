//! zylos-channels — Persistência real de canais e mensagens via SQLite (sqlx).
//!
//! Substitui o backend em memória (DashMap) por um store persistente com
//! migrations individuais. Fornece `ChannelsStore` para uso no gateway.

pub mod models;
pub mod store;

pub use models::{Channel, ChannelMember, ChannelMessage};
pub use store::ChannelsStore;
