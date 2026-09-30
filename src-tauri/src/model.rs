use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeInfo {
    pub id: String,
}

/// Uma mensagem do chat já normalizada para o que o filtro precisa.
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub id: String,
    pub user_login: String,
    pub display_name: String,
    pub text: String,
    pub badges: Vec<BadgeInfo>,
    pub is_mod: bool,
    pub reply: Option<super::filter::ReplyInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moderation {
    MessageDeleted { target_id: String },
    UserBanned { user_login: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "message", rename_all = "camelCase")]
pub enum EngineStatus {
    Idle,
    Connecting,
    Live,
    Stopped,
    Error(String),
}

/// Motivo pelo qual uma mensagem não foi encaminhada. Serve para o registro da interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    BlockedUser,
    RoleNotAllowed,
    Cooldown,
    Duplicate,
    NoApprovedLink,
    DeletedByMod,
    BannedByMod,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counters {
    pub sent: u64,
    pub skipped: u64,
    pub deleted: u64,
    pub banned: u64,
}
