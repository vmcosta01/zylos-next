//! Zylos Cockpit — i18n (pt-BR / en-US)
//!
//! Lazy-loaded translation bundles per DESIGN_SYSTEM.md and Master Plan Task 6.2.1.
//! Defaults to pt-BR; switches at runtime without reload.

use leptos::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    PtBr,
    EnUs,
}

impl Locale {
    pub fn code(&self) -> &'static str {
        match self {
            Locale::PtBr => "pt-BR",
            Locale::EnUs => "en-US",
        }
    }
}

/// Translation key → localized string map
type Bundle = HashMap<&'static str, &'static str>;

fn pt_br_bundle() -> Bundle {
    let mut m = HashMap::new();
    m.insert("app.title", "Zylos Cockpit");
    m.insert("tab.agents", "Agentes");
    m.insert("tab.tasks", "Tarefas");
    m.insert("tab.memory", "Memória");
    m.insert("tab.skills", "Skills");
    m.insert("tab.logs", "Logs");
    m.insert("tab.soar", "SOAR");
    m.insert("health.status", "Status");
    m.insert("health.streams", "Streams Ativos");
    m.insert("health.version", "Versão");
    m.insert("error.connection", "Conexão perdida");
    m.insert("error.retry", "Tentar novamente");
    m.insert("empty.agents", "Nenhum agente configurado");
    m.insert("empty.cta", "Criar Primeiro Agente");
    m.insert("theme.dark", "Escuro");
    m.insert("theme.light", "Claro");
    m.insert("agent.status.idle", "Ocioso");
    m.insert("agent.status.running", "Executando");
    m.insert("agent.status.blocked", "Bloqueado");
    m
}

fn en_us_bundle() -> Bundle {
    let mut m = HashMap::new();
    m.insert("app.title", "Zylos Cockpit");
    m.insert("tab.agents", "Agents");
    m.insert("tab.tasks", "Tasks");
    m.insert("tab.memory", "Memory");
    m.insert("tab.skills", "Skills");
    m.insert("tab.logs", "Logs");
    m.insert("tab.soar", "SOAR");
    m.insert("health.status", "Status");
    m.insert("health.streams", "Active Streams");
    m.insert("health.version", "Version");
    m.insert("error.connection", "Connection lost");
    m.insert("error.retry", "Retry");
    m.insert("empty.agents", "No agents configured");
    m.insert("empty.cta", "Create First Agent");
    m.insert("theme.dark", "Dark");
    m.insert("theme.light", "Light");
    m.insert("agent.status.idle", "Idle");
    m.insert("agent.status.running", "Running");
    m.insert("agent.status.blocked", "Blocked");
    m
}

/// Global i18n context
#[derive(Clone)]
pub struct I18n {
    locale: RwSignal<Locale>,
}

impl I18n {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale: RwSignal::new(locale),
        }
    }

    pub fn locale(&self) -> RwSignal<Locale> {
        self.locale
    }

    pub fn t(&self, key: &'static str) -> String {
        let locale = self.locale.get();
        let bundle = match locale {
            Locale::PtBr => pt_br_bundle(),
            Locale::EnUs => en_us_bundle(),
        };
        bundle
            .get(key)
            .copied()
            .unwrap_or(key)
            .to_string()
    }

    pub fn set_locale(&self, locale: Locale) {
        self.locale.set(locale);
    }
}

/// Provide i18n context to component tree
pub fn provide_i18n(locale: Locale) -> I18n {
    let i18n = I18n::new(locale);
    provide_context(i18n.clone());
    i18n
}

/// Access i18n from any component
pub fn use_i18n() -> I18n {
    expect_context::<I18n>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pt_br_translations() {
        let bundle = pt_br_bundle();
        assert_eq!(bundle.get("tab.agents"), Some(&"Agentes"));
        assert_eq!(bundle.get("error.retry"), Some(&"Tentar novamente"));
    }

    #[test]
    fn test_en_us_translations() {
        let bundle = en_us_bundle();
        assert_eq!(bundle.get("tab.agents"), Some(&"Agents"));
        assert_eq!(bundle.get("error.retry"), Some(&"Retry"));
    }

    #[test]
    fn test_locale_codes() {
        assert_eq!(Locale::PtBr.code(), "pt-BR");
        assert_eq!(Locale::EnUs.code(), "en-US");
    }

    #[test]
    fn test_all_keys_present_in_both_bundles() {
        let pt = pt_br_bundle();
        let en = en_us_bundle();
        for key in pt.keys() {
            assert!(en.contains_key(key), "Missing key {} in en-US bundle", key);
        }
        for key in en.keys() {
            assert!(pt.contains_key(key), "Missing key {} in pt-BR bundle", key);
        }
    }
}