use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Broadcaster,
    Moderator,
    Vip,
    Subscriber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Quem pode ter links repassados. Vazio = qualquer pessoa.
    pub allowed_roles: Vec<Role>,
    /// Bots comuns de chat (sempre ignorados).
    pub blocked_users: Vec<String>,
    /// Encurtadores, rastreadores de IP e convites do Discord.
    pub blocked_domains: Vec<String>,
    /// Quando preenchido, SÓ estes domínios passam.
    pub allowed_domains: Vec<String>,
    /// Ignora o mesmo link repetido dentro desse prazo, em minutos.
    pub dedupe_minutes: u32,
    /// Intervalo mínimo entre mensagens aprovadas do mesmo usuário, em segundos.
    pub user_cooldown_seconds: u32,
    /// Espera antes de postar; se um mod apagar a mensagem nesse tempo, não posta.
    pub delay_seconds: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            allowed_roles: Vec::new(),
            blocked_users: [
                "nightbot",
                "streamelements",
                "streamlabs",
                "moobot",
                "fossabot",
                "wizebot",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            blocked_domains: [
                "bit.ly",
                "tinyurl.com",
                "t.co",
                "grabify.link",
                "iplogger.org",
                "discord.gg",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            allowed_domains: Vec::new(),
            dedupe_minutes: 30,
            user_cooldown_seconds: 20,
            delay_seconds: 8,
        }
    }
}

impl Settings {
    /// Normaliza as listas para que a comparação de usuários e domínios
    /// não dependa de maiúsculas ou espaços deixados na interface.
    pub fn normalized(&self) -> NormalizedSettings {
        NormalizedSettings {
            allowed_roles: self.allowed_roles.clone(),
            blocked_users: self
                .blocked_users
                .iter()
                .map(|u| u.trim().to_lowercase())
                .filter(|u| !u.is_empty())
                .collect(),
            blocked_domains: self
                .blocked_domains
                .iter()
                .map(|d| normalize_domain(d))
                .filter(|d| !d.is_empty())
                .collect(),
            allowed_domains: self
                .allowed_domains
                .iter()
                .map(|d| normalize_domain(d))
                .filter(|d| !d.is_empty())
                .collect(),
            dedupe_minutes: self.dedupe_minutes,
            user_cooldown_seconds: self.user_cooldown_seconds,
            delay_seconds: self.delay_seconds,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedSettings {
    pub allowed_roles: Vec<Role>,
    pub blocked_users: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub dedupe_minutes: u32,
    pub user_cooldown_seconds: u32,
    pub delay_seconds: u32,
}

impl NormalizedSettings {
    /// Vazio = qualquer pessoa passa.
    pub fn roles_restricted(&self) -> bool {
        !self.allowed_roles.is_empty()
    }

    pub fn domain_allowed(&self, host: &str) -> bool {
        if matches_domain(host, &self.blocked_domains) {
            return false;
        }
        if self.allowed_domains.is_empty() {
            return true;
        }
        matches_domain(host, &self.allowed_domains)
    }
}

/// Domínio bloqueado/permitido casa com o host inteiro e com subdomínios.
pub fn matches_domain(host: &str, list: &[String]) -> bool {
    list.iter()
        .any(|d| host == d || host.ends_with(&format!(".{d}")))
}

fn normalize_domain(raw: &str) -> String {
    let d = raw.trim().to_lowercase();
    let d = d.strip_prefix("https://").unwrap_or(&d);
    let d = d.strip_prefix("http://").unwrap_or(d);
    let d = d.strip_prefix("www.").unwrap_or(d);
    d.split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_domains_from_user_input() {
        let s = Settings {
            blocked_domains: vec!["  HTTPS://WWW.Bit.ly/pasta ".into()],
            ..Default::default()
        };
        assert_eq!(s.normalized().blocked_domains, vec!["bit.ly".to_string()]);
    }

    #[test]
    fn domain_match_covers_subdomains() {
        let list = vec!["bit.ly".to_string()];
        assert!(matches_domain("bit.ly", &list));
        assert!(matches_domain("a.b.bit.ly", &list));
        assert!(!matches_domain("notbit.ly", &list));
        assert!(!matches_domain("bit.ly.evil.com", &list));
    }

    #[test]
    fn allow_list_is_exclusive_and_block_list_wins() {
        let s = Settings {
            allowed_domains: vec!["youtube.com".into()],
            blocked_domains: vec!["youtube.com".into()],
            ..Default::default()
        }
        .normalized();
        assert!(!s.domain_allowed("youtube.com"));
    }

    #[test]
    fn empty_lists_mean_no_restriction() {
        let s = Settings {
            allowed_domains: vec![],
            blocked_domains: vec![],
            blocked_users: vec![],
            ..Default::default()
        }
        .normalized();
        assert!(s.domain_allowed("qualquer.coisa"));
        assert!(!s.roles_restricted());
    }

    #[test]
    fn defaults_match_the_previous_version() {
        let s = Settings::default();
        assert_eq!(s.dedupe_minutes, 30);
        assert_eq!(s.delay_seconds, 8);
        assert_eq!(s.user_cooldown_seconds, 20);
        assert!(s.blocked_domains.contains(&"bit.ly".to_string()));
        assert!(s.blocked_domains.contains(&"discord.gg".to_string()));
        assert_eq!(s.blocked_users.len(), 6);
        assert!(s.allowed_roles.is_empty());
        assert!(s.allowed_domains.is_empty());
    }

    #[test]
    fn settings_round_trip_through_json() {
        let s = Settings {
            allowed_roles: vec![Role::Moderator, Role::Vip],
            blocked_users: vec!["bot".into()],
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.allowed_roles, vec![Role::Moderator, Role::Vip]);
        assert_eq!(back.blocked_users, vec!["bot".to_string()]);
    }

    #[test]
    fn missing_json_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"dedupeMinutes": 5}"#).unwrap();
        assert_eq!(s.dedupe_minutes, 5);
        assert_eq!(s.delay_seconds, 8);
        assert_eq!(s.blocked_users.len(), 6);
    }
}
