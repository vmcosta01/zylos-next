//! # zylos-panel
//!
//! Painel Adversarial Deterministico com guardrails, detector de falso-sucesso e votacao por refutacao aterrada.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidencia { Alta, Media, Baixa }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LenteCanonica {
    Funcionalidade,
    Estrutura,
    Performance,
    Seguranca,
    Simplicidade,
    Testes,
}
impl LenteCanonica {
    pub fn alcane(&self) -> &'static str {
        match self {
            Self::Funcionalidade => "funcionalidade",
            Self::Estrutura => "estrutura",
            Self::Performance => "performance",
            Self::Seguranca => "seguranca",
            Self::Simplicidade => "simplicidade",
            Self::Testes => "testes",
        }
    }
    pub fn todas() -> Vec<LenteCanonica> {
        vec![
            LenteCanonica::Funcionalidade,
            LenteCanonica::Estrutura,
            LenteCanonica::Performance,
            LenteCanonica::Seguranca,
            LenteCanonica::Simplicidade,
            LenteCanonica::Testes,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeVote {
    pub judge_id: String,
    pub gerador_id: Option<String>,
    pub aprovado: bool,
    pub refutacao_aterrada: Option<String>,
    pub lente: LenteCanonica,
    pub vestigios: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDecision {
    pub aprovado: bool,
    pub confianca: Confidencia,
    pub votos_totais: usize,
    pub votos_aprovados: usize,
    pub refutacoes_aterradas: usize,
    pub filtrados_auto_julgamento: usize,
    pub guardrail_ativo: bool,
    pub falso_sucesso_detectado: bool,
    pub lentes_cobertas: Vec<LenteCanonica>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub lentes: Vec<LenteCanonica>,
    pub min_votos: usize,
    pub falso_sucesso_min_vestigios: usize,
}
impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            lentes: LenteCanonica::todas(),
            min_votos: 3,
            falso_sucesso_min_vestigios: 1,
        }
    }
}

pub fn filtrar_auto_julgamento(votos: &[JudgeVote]) -> (Vec<JudgeVote>, usize) {
    let mut filtrados = Vec::new();
    let mut count = 0;
    for v in votos {
        if let Some(ref gid) = &v.gerador_id {
            if &v.judge_id == gid { count += 1; continue; }
        }
        filtrados.push(v.clone());
    }
    (filtrados, count)
}

pub fn detector_falso_sucesso(votos: &[JudgeVote], min_vestigios: usize) -> bool {
    let aprovados: Vec<&JudgeVote> = votos.iter().filter(|v| v.aprovado).collect();
    if aprovados.is_empty() { return false; }
    let total_vestigios: usize = aprovados.iter().map(|v| v.vestigios.len()).sum();
    total_vestigios < min_vestigios
}

pub fn guardrail_deterministico(conteudo: &str) -> Option<String> {
    let trimmed = conteudo.trim().to_lowercase();
    if trimmed.is_empty() { return Some("conteudo vazio".into()); }
    if trimmed == "ok" || trimmed == "yes" || trimmed == "true" || trimmed == "sim" {
        return Some("resposta obviamente ruim (sem analise)".into());
    }
    if trimmed.contains("error") || trimmed.contains("fail") || trimmed.contains("falha") {
        return Some("conteudo contem explicito de erro".into());
    }
    None
}

pub fn aggregate_votes(config: &PanelConfig, votos: &[JudgeVote]) -> PanelDecision {
    let (filtrados, auto_filtrados) = filtrar_auto_julgamento(votos);
    let guard = guardrail_deterministico("");
    let falso_sucesso = detector_falso_sucesso(&filtrados, config.falso_sucesso_min_vestigios);
    let total = filtrados.len();
    let aprovados = filtrados.iter().filter(|v| v.aprovado).count();
    let refutacoes = filtrados.iter().filter(|v| !v.aprovado && v.refutacao_aterrada.is_some()).count();
    let lentes_cobertas: Vec<LenteCanonica> = filtrados.iter().map(|v| v.lente.clone()).collect();
    let aprovado = if refutacoes > 0 { false } else if falso_sucesso { false } else { aprovados > 0 };
    let confianca = vote_confidence(total, aprovados, refutacoes);
    PanelDecision {
        aprovado, confianca, votos_totais: total, votos_aprovados: aprovados,
        refutacoes_aterradas: refutacoes, filtrados_auto_julgamento: auto_filtrados,
        guardrail_ativo: guard.is_some(), falso_sucesso_detectado: falso_sucesso, lentes_cobertas,
    }
}

pub fn vote_confidence(total: usize, aprovados: usize, refutacoes: usize) -> Confidencia {
    if total == 0 { return Confidencia::Baixa; }
    if refutacoes > 0 { return Confidencia::Alta; }
    let ratio = aprovados as f64 / total as f64;
    if ratio >= 0.75 { Confidencia::Alta } else if ratio >= 0.5 { Confidencia::Media } else { Confidencia::Baixa }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mk_vote(id: &str, gen: Option<&str>, apr: bool, reff: Option<&str>, lente: LenteCanonica, vest: usize) -> JudgeVote {
        JudgeVote {
            judge_id: id.into(), gerador_id: gen.map(Into::into), aprovado: apr,
            refutacao_aterrada: reff.map(Into::into), lente,
            vestigios: (0..vest).map(|i| format!("v{}", i)).collect(),
        }
    }
    #[test] fn test_guardrail_empty() { assert!(guardrail_deterministico("").is_some()); }
    #[test] fn test_guardrail_obvious_bad() {
        assert!(guardrail_deterministico("ok").is_some());
        assert!(guardrail_deterministico("sim").is_some());
    }
    #[test] fn test_guardrail_error_content() { assert!(guardrail_deterministico("has error here").is_some()); }
    #[test] fn test_guardrail_substantial() { assert!(guardrail_deterministico("analise detalhada do modulo X").is_none()); }
    #[test] fn test_falso_sucesso_sem_vestigios() {
        let v = vec![mk_vote("a1", None, true, None, LenteCanonica::Funcionalidade, 0)];
        assert!(detector_falso_sucesso(&v, 1));
    }
    #[test] fn test_falso_sucesso_com_vestigios() {
        let v = vec![mk_vote("a1", None, true, None, LenteCanonica::Funcionalidade, 2)];
        assert!(!detector_falso_sucesso(&v, 1));
    }
    #[test] fn test_filtrar_auto_julgamento() {
        let v = vec![
            mk_vote("j1", Some("j1"), true, None, LenteCanonica::Funcionalidade, 1),
            mk_vote("j2", Some("j1"), true, None, LenteCanonica::Funcionalidade, 1),
        ];
        let (filtrados, count) = filtrar_auto_julgamento(&v);
        assert_eq!(count, 1); assert_eq!(filtrados.len(), 1); assert_eq!(filtrados[0].judge_id, "j2");
    }
    #[test] fn test_aggregate_refutacao_vence() {
        let v = vec![
            mk_vote("j1", None, true, None, LenteCanonica::Funcionalidade, 1),
            mk_vote("j2", None, true, None, LenteCanonica::Funcionalidade, 1),
            mk_vote("j3", None, false, Some("bug"), LenteCanonica::Funcionalidade, 1),
        ];
        let d = aggregate_votes(&PanelConfig::default(), &v);
        assert!(!d.aprovado); assert_eq!(d.refutacoes_aterradas, 1);
    }
    #[test] fn test_aggregate_unanime_aprovado() {
            let v = vec![mk_vote("j1", None, true, None, LenteCanonica::Funcionalidade, 1)];
            let d = aggregate_votes(&PanelConfig::default(), &v);
            assert!(d.aprovado); assert_eq!(d.confianca, Confidencia::Alta);
        }
    #[test] fn test_aggregate_falso_sucesso_blocks() {
        let v = vec![mk_vote("j1", None, true, None, LenteCanonica::Funcionalidade, 0)];
        let d = aggregate_votes(&PanelConfig::default(), &v);
        assert!(!d.aprovado); assert!(d.falso_sucesso_detectado);
    }
    #[test] fn test_vote_confidence_alta() {
        assert_eq!(vote_confidence(4, 4, 0), Confidencia::Alta);
        assert_eq!(vote_confidence(3, 3, 1), Confidencia::Alta);
    }
    #[test] fn test_vote_confidence_media() { assert_eq!(vote_confidence(4, 2, 0), Confidencia::Media); }
    #[test] fn test_vote_confidence_baixa() {
        assert_eq!(vote_confidence(4, 1, 0), Confidencia::Baixa);
        assert_eq!(vote_confidence(0, 0, 0), Confidencia::Baixa);
    }
    #[test] fn test_lente_alcane() {
        assert_eq!(LenteCanonica::Funcionalidade.alcane(), "funcionalidade");
        assert_eq!(LenteCanonica::Seguranca.alcane(), "seguranca");
    }
    #[test] fn test_lente_todas() { assert_eq!(LenteCanonica::todas().len(), 6); }
    #[test] fn test_panel_config_default() {
        let c = PanelConfig::default(); assert_eq!(c.min_votos, 3); assert_eq!(c.falso_sucesso_min_vestigios, 1);
    }
    #[test] fn test_aggregate_empty() {
        let d = aggregate_votes(&PanelConfig::default(), &[]);
        assert!(!d.aprovado); assert_eq!(d.votos_totais, 0);
    }
    #[test] fn test_guardrail_english() {
        assert!(guardrail_deterministico("YES").is_some());
        assert!(guardrail_deterministico("TRUE").is_some());
    }
    #[test] fn test_guardrail_portuguese() { assert!(guardrail_deterministico("SIM").is_some()); }
}
