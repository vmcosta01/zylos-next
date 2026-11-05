# Engenharia Reversa: langchain-ai/smithtune

**Data:** 2026-11-04
**Repositório:** https://github.com/langchain-ai/smithtune (v0.1.6)
**Objetivo:** Extrair lições arquiteturais aplicáveis aos 6 sistemas core do Zylos-Next.

---

## 1. Arquitetura e Stack Principal

O smithtune é uma CLI Python para pós-treinamento (SFT) de modelos LLM usando trajetórias capturadas no LangSmith. A arquitetura é **orientada a pipeline linear com estado persistente em disco**, não um serviço web ou framework genérico de agentes.

-   **Linguagem:** Python 3.12 (strict), empacotado com `setuptools` e gerenciado via `uv`.
-   **Stack Core:** `langsmith` SDK (dados/traces), `transformers` + `trl` (tokenização/rendering), `fireworks-ai` e `baseten-loops` (treinamento/inferência remota).
-   **Orquestração Local:** `argparse` (CLI), `ThreadPoolExecutor` (concorrência em avaliação/download), `pathlib` + JSON/JSONL (estado).
-   **Dependências Críticas:** `deepagents` (opcional, para council de juízes), `rich` (progresso), `jsonschema` (validação de contratos).
-   **Modelo de Execução:** Processos efêmeros que leem/escrevem em diretórios de trabalho (`data_dir`, `run_dir`). O controller pode ser destacado (`--detach`) para rodar em VM remota sem sessão ativa.

## 2. Componentes Principais e Responsabilidades

| Módulo | Responsabilidade | Lição Arquitetural |
| :--- | :--- | :--- |
| `cli.py` | Parsing argparse, validação de flags, despacho para funções de domínio. | Separação total entre interface e lógica; cada comando retorna um dict JSON serializável. |
| `dataset_workflow.py` | Orquestra pull → triage → push como estágios idempotentes. | Estado congelado por diretório; conflitos de parâmetros forçam novo diretório. |
| `triage_agent.py` / `triage_coordinator.py` | Council de juízes LLM para curadoria de dados. | Votação por maioria estrita; prompts fixos no código, rubrica externa obrigatória. |
| `controller.py` | Loop de treinamento/avaliação destacável, health-check local. | Controller sobrevive a desconexão SSH; logs e status são arquivos, não streams voláteis. |
| `checkpoint.py` | Leitura/escrita atômica de `checkpoint.json` com lock de arquivo. | Recuperação exata de estágios pendentes; nunca sobrescreve progresso parcial. |
| `rendering.py` / `hf_rendering.py` | Conversão trajetória → formato SFT específico do modelo. | Validação rigorosa de contexto/tokenização antes do treino; rejeição inteira em vez de truncamento. |
| `evaluation/replay.py` | Avaliação comparativa base vs tuned via next-action prediction. | Replay determinístico; métricas pareadas (wins/regressions); publicação assíncrona no LangSmith. |
| `inference_contract.py` | Schema de ferramentas disponíveis por mensagem assistente. | Contrato de inferência vinculado a cada exemplo; hash SHA256 garante consistência. |
| `skills/smithtune/SKILL.md` | Instruções operacionais para agentes de codificação. | Skill como fonte única de verdade operacional; alinhamento obrigatório com CLI. |

## 3. Padrões de Orquestração de Agentes/Workflows

O smithtune **não é um framework de agentes autônomos**; é uma ferramenta determinística onde "agentes" aparecem apenas como componentes de validação (council de juízes).

-   **Pipeline Linear com Gates:** `pull → triage → push → prepare → plan → train → evaluate → deploy`. Cada estágio tem preview (`--confirm` ausente) e execução paga.
-   **Council Pattern (Triage):** Múltiplos modelos juízes votam independentemente. Decisão por maioria estrita. Rubrica fornecida pelo usuário, nunca padrão. Juízes recebem trajetória completa e imutável.
-   **Idempotência por Diretório:** Cada diretório de trabalho congela parâmetros (filtro, janela temporal, rubrica, provider). Tentar mudar parâmetros exige novo diretório. Isso elimina ambiguidade de estado.
-   **Preview/Confirm Universal:** Comandos destrutivos ou pagos exigem `--confirm`. Preview mostra exatamente o que será feito (contagens, custos estimados) sem executar.
-   **Skill como Contrato Operacional:** O `SKILL.md` define o fluxo canônico para agentes externos (Claude Code, Codex). A CLI é a implementação; a skill é a especificação executável.

## 4. Sistema de Memória e Estado

Estado é **exclusivamente baseado em sistema de arquivos**, sem banco de dados ou cache distribuído.

-   **Checkpoint Atômico:** `checkpoint.json` grava estágio atual, source frozen, workflow state. Escrita via `output_lock()` (file lock) + write-temp-rename.
-   **Snapshots Imutáveis:** `snapshot.json` + arquivos de trajetória individuais. Hash SHA256 verifica integridade. Triage vota sobre snapshot congelado; novos pulls criam novo snapshot.
-   **Receipts de Upload:** `dataset-import.json` registra IDs de exemplos enviados ao LangSmith. Resume compara receipts com snapshot local para evitar duplicatas.
-   **Run Artifacts:** `plan.json`, `result.json`, `sampler.json`, `summary.json` em `run_dir`. Cada arquivo tem schema versionado e metadados de proveniência (epoch, step, run_id pai).
-   **Recuperação Granular:** `_pending()` calcula estágios incompletos comparando checkpoint, snapshot, summary e receipts. `resume` continua exatamente de onde parou.
-   **Sem Estado Volátil:** Nada em memória sobrevive a restart. Controller destacável escreve tudo em disco antes de cada transição.

## 5. Integrações e Conectores

Arquitetura de **provider abstraction** com contrato comum.

-   **LangSmith (Dados + Observabilidade):** Fonte primária de trajetórias (`/v1/trajectory`), destino de datasets, publicação de experimentos de replay. Autenticação via `LANGSMITH_API_KEY`.
-   **Fireworks AI:** Treinamento LoRA/dedicated, serverless inference, deploy via `firectl` CLI wrapper. Requer `FIREWORKS_API_KEY`.
-   **Baseten:** Treinamento via `baseten-loops`, deploy via `truss` (extra opcional), samplers temporários para avaliação. Requer `BASETEN_API_KEY`.
-   **Anthropic/OpenAI:** Apenas como juízes de avaliação/council, nunca como training backend.
-   **Contrato de Provider:** `providers/base.py` define `CommonSFTSettings`, `TrainingOptions`, `ModelOptions`. Cada provider implementa `prepare()`, `train()`, `deploy()`, `undeploy()`.
-   **Inference Contract:** `inference_contract.py` valida schemas de ferramentas por mensagem. Hash garante que replay usa exatamente as mesmas ferramentas do treino.

## 6. Capacidades de Observabilidade/Debug

Observabilidade é **nativa e estruturada**, não ad-hoc.

-   **Doctor Command:** `smithtune doctor` verifica prereqs locais (keys, dependências, versões) sem chamadas de rede.
-   **JSON Structured Output:** Todo comando imprime JSON no stdout + progresso humano no stderr. Parseável por agentes e scripts.
-   **LangSmith Experiments:** Replay publica automaticamente comparações base-vs-tuned com métricas pareadas. Link de comparação impresso antes da geração.
-   **Download/Triage Summaries:** Contagens detalhadas de exclusões por motivo (`multimodal`, `missing_tool_availability`, `foreign_trace_evidence`).
-   **Controller Logs:** `smithtune logs --follow` stream de controller destacável. `status` reporta saúde local sem consultar provider.
-   **State Inspection:** `dataset resume DIR` mostra estágios pendentes, coleção status, próxima ação sugerida. Sem side effects.
-   **Calibration Checks:** Juiz de avaliação roda casos de calibração antes do scoring real; falha early se juiz estiver misconfigurado.
-   **Advisories:** Mensagens proativas quando dataset é pequeno (<100 trajetórias aprovadas) ou quando collection esgotou candidatos.

## 7. Pontos Fortes e Fracos

### Fortes
-   **State Machine Explícita:** Estado em disco com checksums torna recuperação confiável e auditável.
-   **Safety by Default:** Preview/confirm universal previne acidentes financeiros. Dados rights acknowledgment obrigatório.
-   **Contract Rigor:** Inference contracts com hash evitam drift silencioso entre treino e avaliação.
-   **Provider Abstraction Limpa:** Adicionar novo provider requer implementar interface bem definida; CLI e evaluation não mudam.
-   **Skill-First Design:** Documentação operacional como artefato versionado, não wiki desatualizada.
-   **Determinismo:** Replay é reproduzível; same inputs → same cases → same scores (modulo sampling temperature).

### Fracos
-   **Python-Only Stack:** Dependência pesada de ecossistema Python (transformers, trl, datasets). Incompatível com runtime Rust nativo.
-   **File-Based State:** Não escala para múltiplos workers concorrentes no mesmo dataset. Lock de arquivo é gargalo.
-   **No Streaming/Realtime:** Batch-only. Não suporta fine-tuning contínuo ou online learning.
-   **Council Latency:** Triage sequencial por trajetória; concurrency limitada a 16. Dataset grande = horas de julgamento.
-   **Vendor Lock-in Profundo:** Dependência crítica de LangSmith para dados + avaliação. Fireworks/Baseten são os únicos training backends.
-   **Beta Instability:** Formatos de diretório podem mudar entre releases. Não há migration path garantido.
-   **No Multi-Tenant:** Single-user CLI. Sem isolamento de projetos, quotas, ou auth multi-usuário.

## 8. Lições Aplicáveis ao Zylos-Next

Mapeamento direto para os 6 sistemas core do Zylos-Next (Rust soberano):

### BOT_SYSTEM
-   **Adotar:** Pattern de inference contract com hash SHA256 por tool schema. Garante que bot em produção usa exatamente as ferramentas validadas.
-   **Adaptar:** Council pattern para validação de ações críticas (não para curadoria de dados). Múltiplos verificadores independentes votam antes de executar ação irreversível.
-   **Evitar:** Dependência de LLM como juiz em hot path. Usar apenas para auditoria offline.

### MEMORY_SYSTEM
-   **Adotar:** State machine baseada em arquivos com checksums + atomic writes. Traduzir para Rust como WAL + snapshot com verificação HMAC.
-   **Adaptar:** Receipt-based resume para operações longas. Registrar progresso granular permite recovery sem reprocessamento.
-   **Evitar:** File locks globais. Usar sharding ou MVCC para concorrência real.

### LIVE_COMMUNICATION
-   **Adotar:** Structured JSON output + human-readable stderr separation. Protocolo de comunicação deve ser parseável por máquinas e legível por humanos simultaneamente.
-   **Adaptar:** Preview/confirm como protocolo de handshake para operações custosas. Nunca executar ação irreversível sem ack explícito.
-   **Evitar:** Batch-only design. Live communication exige streaming; adaptar state machine para suportar eventos incrementais.

### VERIFICATION_PROOF
-   **Adotar:** Replay evaluation com métricas pareadas (wins/regressions/ties). Prova de melhoria é comparativa e estatística, não absoluta.
-   **Adaptar:** Calibration checks antes de scoring real. Validar instrumento de medição antes de medir.
-   **Adotar:** Proveniência completa em artifacts (parent_run_id, checkpoint_epoch, contract_hash). Audit trail é dado estrutural, não metadata opcional.
-   **Evitar:** Juiz LLM como única fonte de verdade. Usar como sinal complementar a verificadores determinísticos.

### SANDBOX_ISOLATION
-   **Adotar:** Directory-per-run isolation. Cada execução tem workspace próprio; contaminação cruzada é impossível por design.
-   **Adaptar:** Frozen parameters per sandbox. Configuração é imutável após criação; mudança exige novo sandbox.
-   **Evitar:** Dependência de filesystem local para estado crítico. Em sandbox distribuído, estado deve ser object storage com versionamento.

### SKILLS_REGISTRY
-   **Adotar:** Skill como contrato operacional versionado (SKILL.md). Skills são artefatos de código, não documentação separada.
-   **Adaptar:** Alignment check entre skill spec e implementação. CI deve validar que CLI behavior match skill documentation.
-   **Adotar:** Optional extras pattern (`deepagents`, `baseten-deploy`). Core é mínimo; capacidades especializadas são opt-in com dependências explícitas.
-   **Evitar:** Skills que assumem provider específico. Skills devem ser provider-agnostic ou declarar compatibilidade explicitamente.

### Padrões Transversais Prioritários

1.  **Atomic State Transitions:** Toda mudança de estado é atômica e verificável. Rust: usar `tempfile` + `rename` + fsync.
2.  **Structured Error Taxonomy:** Erros são categorizados (`multimodal`, `missing_tool_availability`, `contract_violation`), não strings livres. Zylos: enum de erros com payload estruturado.
3.  **Preview-Before-Mutate Universal:** Nenhuma operação side-effectful executa sem preview explícito. Safety não é feature, é default.
4.  **Provenance as First-Class Data:** Todo artifact carrega lineage completo. Em Rust, isso é metadata embutida no tipo, não campo opcional.
5.  **Idempotent Resumability:** Qualquer operação interrompida pode ser retomada exatamente de onde parou, sem re-execução desnecessária. Requer state granular e verificável.

---

## Referências

-   Repositório: https://github.com/langchain-ai/smithtune
-   Versão analisada: v0.1.6 (118 commits, beta)
-   Arquivos-chave inspecionados: `cli.py`, `controller.py`, `dataset_workflow.py`, `triage_agent.py`, `triage_coordinator.py`, `checkpoint.py`, `rendering.py`, `evaluation/replay.py`, `inference_contract.py`, `skills/smithtune/SKILL.md`, `AGENTS.md`, `pyproject.toml`
-   Licença: MIT