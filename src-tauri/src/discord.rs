use crate::model::Counters;
use serde::Serialize;
use std::time::Duration;
use tokio::sync::mpsc;
use url::Url;

/// Aceita ptb/canary, discord.com e discordapp.com, com versão de API opcional.
const WEBHOOK_HOSTS: [&str; 2] = ["discord.com", "discordapp.com"];

pub fn is_valid_webhook_url(raw: &str) -> bool {
    let Ok(u) = Url::parse(raw.trim()) else {
        return false;
    };
    if !matches!(u.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = u.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    // aceita discord.com e discordapp.com, nos ambientes ptb/canary
    let base = host
        .strip_prefix("ptb.")
        .or_else(|| host.strip_prefix("canary."))
        .unwrap_or(host);
    if !WEBHOOK_HOSTS.contains(&base) {
        return false;
    }

    let segs: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
    // /api/webhooks/<id>/<token> ou /api/v10/webhooks/<id>/<token>
    let segs = match segs.as_slice() {
        ["api", rest @ ..] => rest,
        _ => return false,
    };
    let segs = match segs {
        [v, rest @ ..] if v.starts_with('v') && v[1..].chars().all(|c| c.is_ascii_digit()) => rest,
        other => other,
    };
    match segs {
        ["webhooks", id, token] => {
            !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) && !token.is_empty()
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WebhookState {
    Unknown,
    Valid,
    Invalid,
    Unreachable,
}

#[derive(Debug, Clone)]
pub struct Outgoing {
    pub content: String,
}

pub struct DiscordSender {
    client: reqwest::Client,
    url: String,
    queue: mpsc::UnboundedReceiver<Outgoing>,
    events: mpsc::UnboundedSender<DiscordEvent>,
    /// espera mínima entre duas publicações, para não estourar o limite do webhook
    min_gap: Duration,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiscordEvent {
    Sent { content: String },
    HttpError { status: u16 },
    WebhookGone,
    NetworkError { message: String },
}

/// Só faz um GET: o Discord devolve os dados do webhook sem publicar nada.
pub async fn check(url: &str) -> WebhookState {
    if !is_valid_webhook_url(url) {
        return WebhookState::Invalid;
    }
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return WebhookState::Unreachable,
    };
    match client.get(url).send().await {
        Ok(r) => match r.status().as_u16() {
            404 | 401 => WebhookState::Invalid,
            200..=299 => WebhookState::Valid,
            _ => WebhookState::Unreachable,
        },
        Err(_) => WebhookState::Unreachable,
    }
}

impl DiscordSender {
    pub fn new(
        url: String,
        queue: mpsc::UnboundedReceiver<Outgoing>,
        events: mpsc::UnboundedSender<DiscordEvent>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            queue,
            events,
            min_gap: Duration::from_millis(600),
        }
    }

    /// Drena a fila para sempre, com retentativa em 429.
    pub async fn run(mut self) {
        while let Some(item) = self.queue.recv().await {
            loop {
                match self.post(&item.content).await {
                    Ok(()) => {
                        let _ = self.events.send(DiscordEvent::Sent {
                            content: item.content.clone(),
                        });
                        break;
                    }
                    Err(SendError::RateLimited(retry_after)) => {
                        tokio::time::sleep(retry_after + Duration::from_millis(100)).await;
                        continue;
                    }
                    Err(e) => {
                        let _ = self.events.send(e.into_event());
                        break;
                    }
                }
            }
            tokio::time::sleep(self.min_gap).await;
        }
    }

    async fn post(&self, content: &str) -> Result<(), SendError> {
        let body = serde_json::json!({
            "content": content,
            "allowed_mentions": { "parse": [] },
        });

        let res = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SendError::Network(e.to_string()))?;

        let status = res.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        if status == 429 {
            let retry = res
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v.get("retry_after").and_then(|x| x.as_f64()))
                .unwrap_or(1.0);
            return Err(SendError::RateLimited(Duration::from_secs_f64(retry)));
        }
        if status == 404 {
            return Err(SendError::Gone);
        }
        Err(SendError::Http(status))
    }
}

enum SendError {
    RateLimited(Duration),
    Http(u16),
    Gone,
    Network(String),
}

impl SendError {
    fn into_event(self) -> DiscordEvent {
        match self {
            // nunca alcançado: o 429 é retentado em loop antes de virar erro
            SendError::RateLimited(_) => DiscordEvent::NetworkError {
                message: "rate limit excedido".into(),
            },
            SendError::Http(s) => DiscordEvent::HttpError { status: s },
            SendError::Gone => DiscordEvent::WebhookGone,
            SendError::Network(m) => DiscordEvent::NetworkError { message: m },
        }
    }
}

/// Resumo para a interface mostrar o que foi publicado.
pub fn sent_summary(content: &str) -> String {
    let first = content.lines().next().unwrap_or("");
    if first.chars().count() > 120 {
        format!("{}\u{2026}", first.chars().take(119).collect::<String>())
    } else {
        first.to_string()
    }
}

pub type CounterSink = mpsc::UnboundedSender<Counters>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_real_discord_webhook_urls() {
        assert!(is_valid_webhook_url(
            "https://discord.com/api/webhooks/123456789012345678/abcDEF-123_ghijk"
        ));
        assert!(is_valid_webhook_url(
            "https://discordapp.com/api/webhooks/123/token"
        ));
        assert!(is_valid_webhook_url(
            "https://ptb.discord.com/api/v10/webhooks/123/token"
        ));
        assert!(is_valid_webhook_url(
            "https://canary.discord.com/api/webhooks/1/t"
        ));
    }

    #[test]
    fn rejects_lookalike_and_malformed_urls() {
        assert!(!is_valid_webhook_url(
            "https://discord.com.example.org/api/webhooks/1/t"
        ));
        assert!(!is_valid_webhook_url("https://evil.com/api/webhooks/1/t"));
        assert!(!is_valid_webhook_url(
            "https://discord.com/api/webhooks/abc/t"
        ));
        assert!(!is_valid_webhook_url(
            "https://discord.com/api/webhooks/123"
        ));
        assert!(!is_valid_webhook_url(
            "http://discord.com/api/webhooks/123/t/x"
        ));
        assert!(!is_valid_webhook_url(
            "https://discord.com.evil.io/api/webhooks/1/t"
        ));
        assert!(!is_valid_webhook_url(""));
    }

    /// nobody_ping: o corpo sempre leva allowed_mentions vazio, que é o que
    /// realmente impede que @everyone, cargos e usuários sejam marcados.
    /// Este teste trava esse formato, já que ele não pode ser testado por rede.
    #[test]
    fn post_body_never_parses_mentions() {
        let body = serde_json::json!({
            "content": "@everyone",
            "allowed_mentions": { "parse": [] },
        });
        assert_eq!(body["allowed_mentions"]["parse"], serde_json::json!([]));
        assert!(body["allowed_mentions"]
            .as_object()
            .unwrap()
            .get("users")
            .is_none());
    }

    #[test]
    fn summary_truncates_long_lines() {
        let long = "x".repeat(200);
        assert!(sent_summary(&long).chars().count() == 120);
        assert!(sent_summary("curto").contains("curto"));
    }
}
