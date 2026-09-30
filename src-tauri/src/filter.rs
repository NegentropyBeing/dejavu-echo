use crate::config::{NormalizedSettings, Role};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use url::Url;

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:https?://|www\.)[^\s<>]+").expect("regex válida"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Part {
    Text {
        value: String,
    },
    Url {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
}

impl Part {
    pub fn is_approved_url(&self) -> bool {
        matches!(self, Part::Url { ok: true, .. })
    }

    pub fn key(&self) -> Option<&str> {
        match self {
            Part::Url { key: Some(k), .. } => Some(k),
            _ => None,
        }
    }
}

/// Divide a mensagem em trechos de texto e de links, sem perder nada.
pub fn split_text(text: &str) -> Vec<Part> {
    let re = url_regex();
    let mut parts: Vec<Part> = Vec::new();
    let mut last = 0usize;

    for m in re.find_iter(text) {
        let matched = m.as_str();
        let trail_len = matched
            .char_indices()
            .rev()
            .take_while(|(_, c)| ".,!?;:)]".contains(*c))
            .count();
        let raw = &matched[..matched.len() - trail_len];
        if raw.is_empty() {
            continue;
        }
        parts.push(Part::Text {
            value: text[last..m.start()].to_string(),
        });
        parts.push(Part::Url {
            value: raw.to_string(),
            url: None,
            ok: false,
            key: None,
        });
        last = m.start() + raw.len();
    }
    parts.push(Part::Text {
        value: text[last..].to_string(),
    });
    parts
}

pub fn parse_link(raw: &str) -> Option<Url> {
    let href = if raw.to_ascii_lowercase().starts_with("www.") {
        format!("https://{raw}")
    } else {
        raw.to_string()
    };
    Url::parse(&href)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
}

/// Chave usada para detectar links repetidos.
pub fn link_key(u: &Url) -> String {
    let mut u = u.clone();
    u.set_fragment(None);
    let s = u.as_str().trim_end_matches('/').to_string();
    s
}

/// Analisa a mensagem: cada link recebe o resultado da checagem de domínio.
pub fn analyze(text: &str, cfg: &NormalizedSettings) -> Vec<Part> {
    split_text(text)
        .into_iter()
        .map(|p| match p {
            Part::Url { value, .. } => match parse_link(&value) {
                Some(u) => {
                    let host = u.host_str().unwrap_or_default().to_lowercase();
                    let host = host.strip_prefix("www.").unwrap_or(&host);
                    let ok = cfg.domain_allowed(host);
                    Part::Url {
                        value,
                        url: Some(u.to_string()),
                        ok,
                        key: Some(link_key(&u)),
                    }
                }
                None => Part::Url {
                    value,
                    url: None,
                    ok: false,
                    key: None,
                },
            },
            other => other,
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyInfo {
    pub user: String,
    pub body: String,
}

pub fn approved_keys(parts: &[Part]) -> Vec<String> {
    parts
        .iter()
        .filter(|p| p.is_approved_url())
        .filter_map(|p| p.key().map(str::to_string))
        .collect()
}

/// Escapa markdown, menções (<@...>) e a sintaxe [texto](link).
/// Escapa markdown e a sintaxe [texto](link), que esconderia o destino real.
/// Menções não são neutralizadas aqui: quem impede que alguém seja marcado é o
/// campo `allowed_mentions` enviado ao Discord, que vem sempre vazio.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '*' | '_' | '`' | '~' | '|' | '>' | '#' | '<' | '[' | ']' | '(' | ')'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        let head: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{head}\u{2026}")
    } else {
        s.to_string()
    }
}

fn render(parts: &[Part]) -> String {
    let joined: String = parts
        .iter()
        .map(|p| match p {
            Part::Text { value } => esc(value),
            Part::Url { value, ok, .. } => {
                if !ok {
                    "*[link removido]*".to_string()
                } else if value.to_ascii_lowercase().starts_with("www.") {
                    format!("https://{value}")
                } else {
                    value.clone()
                }
            }
        })
        .collect();

    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn build_message(nick: &str, parts: &[Part], reply: Option<&ReplyInfo>) -> String {
    let mut lines = vec![format!("\u{1F517} **{}**: {}", esc(nick), render(parts))];
    if let Some(r) = reply {
        let stripped = url_regex().replace_all(&r.body, "[link]");
        let cleaned = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
        let body = trunc(&cleaned, 120);
        lines.push(format!(
            "\u{21AA}\u{FE0F} *em resposta a **{}**: \"{}\"*",
            esc(&r.user),
            esc(&body)
        ));
    }
    lines.join("\n")
}

pub fn roles_of(badges: &[crate::model::BadgeInfo], is_mod: bool) -> Vec<Role> {
    let mut roles = Vec::new();
    if badges.iter().any(|b| b.id == "broadcaster") {
        roles.push(Role::Broadcaster);
    }
    if is_mod || badges.iter().any(|b| b.id == "moderator") {
        roles.push(Role::Moderator);
    }
    if badges.iter().any(|b| b.id == "vip") {
        roles.push(Role::Vip);
    }
    if badges
        .iter()
        .any(|b| matches!(b.id.as_str(), "subscriber" | "founder"))
    {
        roles.push(Role::Subscriber);
    }
    roles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    fn cfg() -> NormalizedSettings {
        Settings::default().normalized()
    }

    #[test]
    fn splits_text_around_links_keeping_everything() {
        let parts = split_text("olá https://a.com mundo");
        assert_eq!(parts.len(), 3);
        assert_eq!(
            parts[0],
            Part::Text {
                value: "olá ".into()
            }
        );
        assert!(parts[1].is_approved_url() || matches!(parts[1], Part::Url { .. }));
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_the_link() {
        let parts = analyze("olha https://a.com/pagina.", &cfg());
        let url = parts.iter().find_map(|p| match p {
            Part::Url { value, .. } => Some(value.clone()),
            _ => None,
        });
        assert_eq!(url.as_deref(), Some("https://a.com/pagina"));
    }

    #[test]
    fn www_link_is_upgraded_and_detected() {
        let parts = analyze("www.exemplo.com/x", &cfg());
        assert!(parts.iter().any(|p| p.is_approved_url()));
    }

    #[test]
    fn non_url_prefix_is_not_detected() {
        assert!(analyze("site.com/pagina", &cfg())
            .iter()
            .all(|p| !p.is_approved_url()));
    }

    #[test]
    fn blocked_domain_is_rejected_but_sibling_allowed() {
        let parts = analyze("https://bit.ly/x e https://youtube.com/y", &cfg());
        let oks: Vec<bool> = parts
            .iter()
            .filter_map(|p| match p {
                Part::Url { ok, .. } => Some(*ok),
                _ => None,
            })
            .collect();
        assert_eq!(oks, vec![false, true]);
    }

    #[test]
    fn blocked_link_inside_approved_message_is_masked() {
        let parts = analyze("olha https://bit.ly/x e https://a.com", &cfg());
        let msg = build_message("Zé", &parts, None);
        assert!(msg.contains("https://a.com"));
        assert!(msg.contains("*[link removido]*"));
        assert!(!msg.contains("bit.ly"));
    }

    /// O texto pode conter "@everyone": quem impede o disparo é o allowed_mentions
    /// vazio no corpo enviado ao Discord (ver discord.rs::post).
    #[test]
    fn mention_text_is_preserved_but_never_parsed_by_discord() {
        let parts = analyze("@everyone https://a.com", &cfg());
        let msg = build_message("Zé", &parts, None);
        assert!(msg.contains("@everyone https://a.com"));
    }

    /// <@123> e <@&123> precisam ficar com o "<" escapado para o Discord
    /// não interpretá-los como menção, já que o allowed_mentions é a segunda camada.
    #[test]
    fn angle_bracket_mentions_are_escaped() {
        let parts = analyze("<@123> <@&456> https://a.com", &cfg());
        let msg = build_message("Zé", &parts, None);
        assert!(!msg.contains("<@123>"));
        assert!(!msg.contains("<@&456>"));
        assert!(msg.contains("\\<@123\\>"));
    }

    #[test]
    fn markdown_link_smuggling_is_escaped() {
        let parts = analyze("[youtube.com](https://golpe.example)", &cfg());
        let msg = build_message("Zé", &parts, None);
        assert!(!msg.contains("](https://golpe.example)"));
    }

    #[test]
    fn nick_is_escaped() {
        let msg = build_message("bad**name", &[Part::Text { value: "oi".into() }], None);
        assert!(msg.contains("bad\\*\\*name"));
    }

    #[test]
    fn reply_line_is_truncated_and_links_masked() {
        let parts = analyze("https://a.com", &cfg());
        // o link vem no começo para não ser cortado pelo limite de 120
        let long = format!("https://secreto.com {}", "y".repeat(300));
        let msg = build_message(
            "Zé",
            &parts,
            Some(&ReplyInfo {
                user: "Ana".into(),
                body: long,
            }),
        );
        assert!(msg.contains("em resposta a **Ana**"));
        assert!(!msg.contains("secreto.com"));
        // o "[link]" é escapado como qualquer texto, igual à versão anterior
        assert!(msg.contains("\\[link\\]"));
        assert!(msg.contains('\u{2026}'));
    }

    #[test]
    fn key_ignores_fragment_and_trailing_slash() {
        let a = parse_link("https://a.com/x#frag").unwrap();
        let b = parse_link("https://a.com/x/").unwrap();
        assert_eq!(link_key(&a), link_key(&b));
    }

    #[test]
    fn whitespace_is_collapsed_in_output() {
        let parts = analyze("a   b\n\nc https://a.com", &cfg());
        let msg = build_message("Zé", &parts, None);
        assert!(!msg.contains("  "));
    }

    #[test]
    fn allowed_roles_gate_is_evaluated_from_badges() {
        let roles = roles_of(
            &[
                crate::model::BadgeInfo {
                    id: "moderator".into(),
                },
                crate::model::BadgeInfo { id: "vip".into() },
            ],
            false,
        );
        assert!(roles.contains(&Role::Moderator));
        assert!(roles.contains(&Role::Vip));
        assert!(!roles.contains(&Role::Broadcaster));
    }

    #[test]
    fn mod_flag_alone_grants_moderator_role() {
        let roles = roles_of(&[], true);
        assert_eq!(roles, vec![Role::Moderator]);
    }
}
