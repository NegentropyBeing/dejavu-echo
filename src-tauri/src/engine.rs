use crate::config::NormalizedSettings;
use crate::filter;
use crate::model::{Counters, IncomingMessage, Moderation, SkipReason};
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct Pending {
    user_login: String,
    fire_at: Instant,
    /// Chaves dos links aprovados, resolvidas no momento da aprovação.
    approved: Vec<String>,
    content: String,
}

#[derive(Debug, Default, Clone)]
pub struct CountersInner {
    pub sent: u64,
    pub skipped: u64,
    pub deleted: u64,
    pub banned: u64,
}

impl CountersInner {
    pub fn as_counters(&self) -> Counters {
        Counters {
            sent: self.sent,
            skipped: self.skipped,
            deleted: self.deleted,
            banned: self.banned,
        }
    }
}

/// Estado do filtro entre mensagens: dedupe, cooldown e o que está esperando o atraso.
pub struct EngineState {
    settings: NormalizedSettings,
    seen_links: HashMap<String, Instant>,
    last_by_user: HashMap<String, Instant>,
    pending: HashMap<String, Pending>,
    pub counters: CountersInner,
}

/// Resultado do processamento de uma mensagem.
pub enum Outcome {
    /// Deve ser publicado no Discord, após o atraso configurado.
    Scheduled {
        fire_at: Instant,
    },
    Skipped(SkipReason),
}

impl EngineState {
    pub fn new(settings: NormalizedSettings) -> Self {
        Self {
            settings,
            seen_links: HashMap::new(),
            last_by_user: HashMap::new(),
            pending: HashMap::new(),
            counters: CountersInner::default(),
        }
    }

    pub fn settings(&self) -> &NormalizedSettings {
        &self.settings
    }

    pub fn replace_settings(&mut self, settings: NormalizedSettings) {
        self.settings = settings;
    }

    fn is_duplicate(&self, key: &str, now: Instant) -> bool {
        match self.seen_links.get(key) {
            Some(t) => {
                now.duration_since(*t)
                    < Duration::from_secs(u64::from(self.settings.dedupe_minutes) * 60)
            }
            None => false,
        }
    }

    /// Decide se a mensagem é encaminhada. Não publica nada: apenas registra o pendente.
    pub fn accept(&mut self, msg: &IncomingMessage, now: Instant) -> Outcome {
        let cfg = self.settings.clone();

        if cfg.blocked_users.contains(&msg.user_login.to_lowercase()) {
            return Outcome::Skipped(SkipReason::BlockedUser);
        }

        let parts = filter::analyze(&msg.text, &cfg);
        let keys = filter::approved_keys(&parts);
        if keys.is_empty() {
            return Outcome::Skipped(SkipReason::NoApprovedLink);
        }

        if cfg.roles_restricted() {
            let roles = filter::roles_of(&msg.badges, msg.is_mod);
            if !roles.iter().any(|r| cfg.allowed_roles.contains(r)) {
                return Outcome::Skipped(SkipReason::RoleNotAllowed);
            }
        }

        // sem registro anterior, o cooldown não se aplica
        if let Some(&last) = self.last_by_user.get(&msg.user_login) {
            if now.duration_since(last) < Duration::from_secs(u64::from(cfg.user_cooldown_seconds))
            {
                return Outcome::Skipped(SkipReason::Cooldown);
            }
        }

        if keys.iter().all(|k| self.is_duplicate(k, now)) {
            return Outcome::Skipped(SkipReason::Duplicate);
        }

        let nick = if msg.display_name.is_empty() {
            msg.user_login.clone()
        } else {
            msg.display_name.clone()
        };
        let content = filter::build_message(&nick, &parts, msg.reply.as_ref());

        self.last_by_user.insert(msg.user_login.clone(), now);
        let fire_at = now + Duration::from_secs(u64::from(cfg.delay_seconds));
        self.pending.insert(
            msg.id.clone(),
            Pending {
                user_login: msg.user_login.clone(),
                fire_at,
                approved: keys,
                content,
            },
        );

        Outcome::Scheduled { fire_at }
    }

    /// Coleta o que já passou do atraso e não foi cancelado.
    pub fn due(&mut self, now: Instant) -> Vec<String> {
        let ids: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, p)| p.fire_at <= now)
            .map(|(id, _)| id.clone())
            .collect();
        ids
    }

    /// Marca os links como vistos e devolve o conteúdo a publicar.
    pub fn commit(&mut self, id: &str, now: Instant) -> Option<String> {
        let p = self.pending.remove(id)?;
        if p.approved.iter().all(|k| self.is_duplicate(k, now)) {
            return None;
        }
        for k in &p.approved {
            self.seen_links.insert(k.clone(), now);
        }
        self.counters.sent += 1;
        Some(p.content)
    }

    /// Cancela o que está pendente. Devolve quantos itens foram afetados.
    pub fn cancel(&mut self, ev: &Moderation) -> usize {
        let ids: Vec<String> = self
            .pending
            .iter()
            .filter(|(id, p)| match ev {
                // paridade com a versão anterior: casa o id exato da mensagem removida
                Moderation::MessageDeleted { target_id } => *id == target_id,
                Moderation::UserBanned { user_login } => &p.user_login == user_login,
            })
            .map(|(id, _)| id.clone())
            .collect();

        let n = ids.len();
        match ev {
            Moderation::MessageDeleted { .. } => self.counters.deleted += n as u64,
            Moderation::UserBanned { .. } => self.counters.banned += n as u64,
        }
        for id in ids {
            self.pending.remove(&id);
        }
        n
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Role;

    fn msg(id: &str, user: &str, text: &str) -> IncomingMessage {
        IncomingMessage {
            id: id.into(),
            user_login: user.into(),
            display_name: user.into(),
            text: text.into(),
            badges: vec![],
            is_mod: false,
            reply: None,
        }
    }

    fn engine(cfg: NormalizedSettings) -> EngineState {
        EngineState::new(cfg)
    }

    #[test]
    fn blocked_user_is_skipped() {
        let mut e = engine(crate::config::Settings::default().normalized());
        let m = msg("1", "nightbot", "https://a.com");
        assert!(matches!(
            e.accept(&m, Instant::now()),
            Outcome::Skipped(SkipReason::BlockedUser)
        ));
    }

    #[test]
    fn message_without_links_is_skipped() {
        let mut e = engine(crate::config::Settings::default().normalized());
        let m = msg("1", "ana", "só um oi");
        assert!(matches!(
            e.accept(&m, Instant::now()),
            Outcome::Skipped(SkipReason::NoApprovedLink)
        ));
    }

    #[test]
    fn role_gate_rejects_regular_user() {
        let cfg = crate::config::Settings {
            allowed_roles: vec![Role::Moderator],
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let m = msg("1", "ana", "https://a.com");
        assert!(matches!(
            e.accept(&m, Instant::now()),
            Outcome::Skipped(SkipReason::RoleNotAllowed)
        ));
    }

    #[test]
    fn role_gate_accepts_moderator() {
        let cfg = crate::config::Settings {
            allowed_roles: vec![Role::Moderator],
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let mut m = msg("1", "ana", "https://a.com");
        m.is_mod = true;
        assert!(matches!(
            e.accept(&m, Instant::now()),
            Outcome::Scheduled { .. }
        ));
    }

    #[test]
    fn cooldown_blocks_second_message_of_same_user() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 20,
            delay_seconds: 0,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        assert!(matches!(
            e.accept(&msg("1", "ana", "https://a.com"), t0),
            Outcome::Scheduled { .. }
        ));
        e.commit("1", t0);
        assert!(matches!(
            e.accept(
                &msg("2", "ana", "https://b.com"),
                t0 + Duration::from_secs(5)
            ),
            Outcome::Skipped(SkipReason::Cooldown)
        ));
    }

    #[test]
    fn cooldown_expires_after_window() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 20,
            delay_seconds: 0,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        e.commit("1", t0);
        assert!(matches!(
            e.accept(
                &msg("2", "ana", "https://b.com"),
                t0 + Duration::from_secs(21)
            ),
            Outcome::Scheduled { .. }
        ));
    }

    #[test]
    fn dedupe_blocks_same_link_inside_window() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 0,
            dedupe_minutes: 30,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        e.commit("1", t0);
        assert!(matches!(
            e.accept(
                &msg("2", "bob", "https://a.com"),
                t0 + Duration::from_secs(10)
            ),
            Outcome::Skipped(SkipReason::Duplicate)
        ));
    }

    #[test]
    fn dedupe_expires_after_window() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 0,
            dedupe_minutes: 30,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        e.commit("1", t0);
        assert!(matches!(
            e.accept(
                &msg("2", "bob", "https://a.com"),
                t0 + Duration::from_secs(31 * 60)
            ),
            Outcome::Scheduled { .. }
        ));
    }

    #[test]
    fn delay_holds_message_until_due() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 8,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        assert!(e.due(t0 + Duration::from_secs(7)).is_empty());
        assert_eq!(e.due(t0 + Duration::from_secs(8)), vec!["1".to_string()]);
    }

    #[test]
    fn mod_deletion_cancels_pending_message() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 30,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("abc", "ana", "https://a.com"), t0);
        let n = e.cancel(&Moderation::MessageDeleted {
            target_id: "abc".into(),
        });
        assert_eq!(n, 1);
        assert_eq!(e.pending_len(), 0);
        assert!(e.commit("abc", t0 + Duration::from_secs(60)).is_none());
    }

    #[test]
    fn ban_cancels_pending_message_of_that_user() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 30,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        e.accept(&msg("2", "bob", "https://b.com"), t0);
        assert_eq!(
            e.cancel(&Moderation::UserBanned {
                user_login: "ana".into()
            }),
            1
        );
        assert_eq!(e.pending_len(), 1);
        assert!(e.commit("2", t0 + Duration::from_secs(60)).is_some());
    }

    #[test]
    fn second_poster_of_same_link_wins_when_first_is_still_waiting() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 8,
            dedupe_minutes: 30,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        let t1 = t0 + Duration::from_secs(2);
        e.accept(&msg("2", "bob", "https://a.com"), t1);
        // os dois vencem quase juntos; o primeiro a ser confirmado vê o link já visto
        let t2 = t0 + Duration::from_secs(9);
        assert!(e.commit("1", t2).is_some());
        assert!(e.commit("2", t2).is_none());
    }

    #[test]
    fn counters_track_sends_and_cancellations() {
        let cfg = crate::config::Settings {
            user_cooldown_seconds: 0,
            delay_seconds: 30,
            ..Default::default()
        }
        .normalized();
        let mut e = engine(cfg);
        let t0 = Instant::now();
        e.accept(&msg("1", "ana", "https://a.com"), t0);
        e.accept(&msg("2", "bob", "https://b.com"), t0);
        e.commit("1", t0 + Duration::from_secs(31));
        e.cancel(&Moderation::UserBanned {
            user_login: "bob".into(),
        });
        assert_eq!(e.counters.sent, 1);
        assert_eq!(e.counters.banned, 1);
        assert_eq!(e.counters.as_counters().sent, 1);
    }
}
