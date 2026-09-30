use crate::model::{BadgeInfo, IncomingMessage, Moderation};
use tokio::sync::mpsc;
use twitch_irc::client::TwitchIRCClient;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{ClearChatAction, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;

pub type Events = mpsc::UnboundedSender<Event>;
pub type Client = TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;

#[derive(Debug, Clone)]
pub enum Event {
    Message(IncomingMessage),
    Moderation(Moderation),
    /// O servidor confirmou a entrada no canal.
    Joined(String),
    Disconnected(String),
}

/// Liga ao chat da Twitch em modo anônimo e traduz as mensagens para o formato interno.
/// Devolve o handle para o chamador, que deve mantê-lo vivo: ao descartá-lo, a conexão fecha.
pub fn spawn(channel: String, tx: Events) -> Client {
    let config = ClientConfig::default();
    let (mut incoming, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

    let login = channel.to_lowercase();
    if let Err(e) = client.join(login.clone()) {
        let _ = tx.send(Event::Disconnected(format!(
            "não consegui entrar no canal {login}: {e}"
        )));
    }

    tauri::async_runtime::spawn(async move {
        while let Some(parsed) = incoming.recv().await {
            match parsed {
                ServerMessage::Privmsg(p) => {
                    let reply = p.reply_parent.as_ref().map(|r| crate::filter::ReplyInfo {
                        user: r.reply_parent_user.name.clone(),
                        body: r.message_text.clone(),
                    });

                    // o tag `mod=1` cobre o mod sem badge (ex.: até o mod receber badge)
                    let is_mod = p.source.tags.0.get("mod").is_some_and(|v| v == "1")
                        || p.badges.iter().any(|b| b.name == "moderator");

                    let _ = tx.send(Event::Message(IncomingMessage {
                        id: p.message_id.clone(),
                        user_login: p.sender.login.clone(),
                        display_name: p.sender.name.trim().to_string(),
                        text: p.message_text.clone(),
                        badges: p
                            .badges
                            .iter()
                            .map(|b| BadgeInfo { id: b.name.clone() })
                            .collect(),
                        is_mod,
                        reply,
                    }));
                }
                ServerMessage::ClearMsg(c) => {
                    let _ = tx.send(Event::Moderation(Moderation::MessageDeleted {
                        target_id: c.message_id.clone(),
                    }));
                }
                ServerMessage::ClearChat(c) => match &c.action {
                    ClearChatAction::UserBanned { user_login, .. }
                    | ClearChatAction::UserTimedOut { user_login, .. } => {
                        let _ = tx.send(Event::Moderation(Moderation::UserBanned {
                            user_login: user_login.clone(),
                        }));
                    }
                    // limpar o chat inteiro não cancela nada: os ids antigos não são reenviados
                    ClearChatAction::ChatCleared => {}
                },
                ServerMessage::Join(j) => {
                    let _ = tx.send(Event::Joined(j.channel_login.clone()));
                }
                _ => {}
            }
        }
        let _ = tx.send(Event::Disconnected(
            "conexão com o chat encerrada".to_string(),
        ));
    });

    client
}

/// Twitch aceita apenas estes caracteres em nomes de canal.
pub fn is_valid_channel(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 25
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Aceita "canal", "#canal" e "https://www.twitch.tv/canal".
pub fn normalize_channel_input(raw: &str) -> String {
    let s = raw.trim().to_ascii_lowercase();
    let s = s
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let s = s.strip_prefix("twitch.tv/").unwrap_or(s);
    // o # do IRC vem antes de qualquer barra
    let s = s.trim_start_matches('#');
    s.split(['/', '?']).next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_and_hashed_names() {
        assert!(is_valid_channel("canal"));
        assert!(is_valid_channel("canal_1"));
        assert!(!is_valid_channel(""));
        assert!(!is_valid_channel("Canal"));
        assert!(!is_valid_channel("canal-1"));
        assert!(!is_valid_channel(&"a".repeat(26)));
    }

    #[test]
    fn normalizes_every_accepted_input_form() {
        assert_eq!(normalize_channel_input("  MeuCanal "), "meucanal");
        assert_eq!(normalize_channel_input("#meucanal"), "meucanal");
        assert_eq!(
            normalize_channel_input("https://www.twitch.tv/meucanal"),
            "meucanal"
        );
        assert_eq!(
            normalize_channel_input("twitch.tv/meucanal?x=1"),
            "meucanal"
        );
    }
}
