//! Modelos de dados para o sistema de canais.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Canal de mensagens persistente.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(serialize_with = "ser_bool", deserialize_with = "de_bool")]
    pub is_private: bool,
    pub created_at: String,
    pub last_message_at: String,
}

/// Contagem de membros (derivada via query).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelWithCount {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_private: i32,
    pub created_at: String,
    pub last_message_at: String,
    pub member_count: i64,
}

impl From<ChannelWithCount> for ChannelResponse {
    fn from(c: ChannelWithCount) -> Self {
        ChannelResponse {
            id: c.id,
            name: c.name,
            description: c.description,
            member_count: c.member_count as usize,
            is_private: c.is_private != 0,
            unread_count: 0,
            last_message_at: c.last_message_at,
        }
    }
}

/// Resposta API compatível com o cockpit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub member_count: usize,
    pub is_private: bool,
    pub unread_count: usize,
    pub last_message_at: String,
}

/// Mensagem em um canal.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct ChannelMessage {
    pub id: String,
    pub channel_id: String,
    pub author: String,
    pub content: String,
    pub timestamp: String,
}

/// Membro de um canal.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct ChannelMember {
    pub channel_id: String,
    pub user_id: String,
    pub joined_at: String,
}

fn ser_bool<S>(v: &bool, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_i32(if *v { 1 } else { 0 })
}

fn de_bool<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = i32::deserialize(d)?;
    Ok(v != 0)
}