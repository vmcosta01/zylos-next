# Implementation Plan — Zylos-Next v6.0

**Referência:** Este documento é um ponteiro para o Plano Mestre completo.

## Localização do Plano Mestre

O plano mestre detalhado com todas as fases, tasks, critérios de aceite e estimativas está em:
- **Desenvolvimento local:** `C:\Users\Paulo\AppData\Local\hermes\cache\scratch\zylos-next-master-plan.md`
- **Versão no repositório:** Deve ser copiada para `docs/MASTER_PLAN.md` após aprovação

## Resumo das Fases

| Fase | Nome | Semanas | Status |
|------|------|---------|--------|
| 0 | Fundação | 1-2 | ✅ Completa |
| 1 | MEMORY_SYSTEM | 3-10 | ✅ Completa |
| 2 | BOT_SYSTEM | 11-18 | ✅ Completa |
| 3 | VERIFICATION_PROOF | 19-24 | ✅ Completa |
| 4 | SANDBOX_ISOLATION | 25-32 | ✅ Completa |
| 5 | SKILLS_REGISTRY | 33-38 | ✅ Completa |
| 6 | LIVE_COMMUNICATION | 39-44 | ✅ Completa |
| 7B | HITL + Task Durability | 45-46 | ✅ Completa |
| 7C | SOAR + Sentinel | 47-48 | ✅ Completa |
| 7 | Integração Final | 49-50 | 🔄 Em Andamento |

## Conceitos do Manifesto Aplicados

Este plano segue obrigatoriamente o [Manifesto Conceitos-Desenvolvimento-IA v3.1](./docs/CONCEITOS-DESENVOLVIMENTO-IA.md) com suas 10 camadas obrigatórias (22 conceitos canônicos + 7 princípios anti-slop + loop profissional + penalidade + arquivos obrigatórios + templates de revisão + matriz de rastreabilidade + integração com harness + 28 skills Hermes + protocolo de evolução).

## Skills Hermes de Auto-Invocação

Todas as 28 skills documentais são invocadas automaticamente pelos agentes conforme gatilhos definidos em AGENTS.md. Ver tabela completa de gatilhos na seção "Skills Documentais — Invocação Automática e Obrigatória".

## Loop Profissional Obrigatório

`READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS`

Aplicado a toda task deste plano sem exceção.
