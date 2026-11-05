# Auditoria Adversarial Independente — Zylos-Next (Fases 0–2)

**Auditor:** agente adversarial independente (instância isolada, fora da cadeia v1→v2→v3)
**Alvo:** `C:/Users/Paulo/zylos-next`
**Manifesto:** `C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md` (v3.0 — 22 conceitos + 7 anti-slop)
**Data:** 2026-10-30 06:18 (-03:00)
**Snapshot:** MEMORY.md `bff061e2…`, HANDOFF_0.md `1f9a7a9b…`, HANDOFF_1.md `32194d89…`, HANDOFF_2.md `1a164178…`, TASKS.md `317aa98e…`, EVALUATION.md `27a38f57…`, TESTING.md `4a1ec5be…`, AGENTS.md `edd1e201…`, `evals/audits/fase2-audit-adversarial-minimax.md` `5a2b09d5…`, `crates/zylos-memory-system/src/context_graph.rs` `d1ac01a9…`

---

## VEREDITO: ❌ CHANGES_REQUIRED

O projeto **não pode** ser declarado Fase 2 concluída. Não é parecer estético: existem blockers reproduzíveis com comando, exit code e linha de arquivo.

Motivos em ordem de gravidade:

1. **Zero commits** — o repositório inteiro está untracked (26 entradas `??`), branch `master` sem nenhum commit, sem remote. Conceito #15 (Git é rede de segurança) violado; Master Plan Task 0.1.6 ("commit inicial na main com CI verde") não cumprido. Não existe histórico, não existe baseline, não existe rollback.
2. **Gates do CI estão vermelhos** — `cargo clippy --workspace --all-targets -- -D warnings` → exit **101** (`clippy::len_zero` em `crates/zylos-memory-system/src/context_graph.rs:715`); `cargo fmt --all -- --check` → exit **1** (113 diffs em 7 arquivos). O CI define `RUSTFLAGS: "-Dwarnings"` (`.github/workflows/ci.yml:11`) e jobs `fmt` e `clippy`. Green local não existe.
3. **Auditoria "isolada de verificação" fantasma** — `HANDOFF_2.md:58` referencia `evals/audits/fase2-audit-v2.md` como "✅ APPROVED". Esse arquivo **não existe**. `ls evals/audits/` retorna apenas `fase0-fase1-audit.md`, `fase0-fase1-audit-v2.md`, `fase0-fase1-audit-v3-glm.md`, `fase0-fase1-audit-v3-minimax.md`, `fase2-audit-adversarial-minimax.md`, `GOAL-CONTRACT-verify-20261029-211946.log`. AGENTS.md:106-108 exige os **dois** relatórios (`faseN-audit.md` + `faseN-audit-v2.md`) APPROVED para declarar a fase concluída — **nenhum dos dois existe com esse nome**.
4. **Relatório de auditoria internamente contraditório** — `evals/audits/fase2-audit-adversarial-minimax.md:11` diz "## Veredito: ✅ APPROVED (Post-Fix)" e `:14` "Todos os blockers resolvidos", mas a assinatura em `:303-309` diz **"❌ CHANGES_REQUIRED"**, "Conceitos Violados: #6, #9, #17, #22", "Anti-Slop Violados: #1, #6"; §1.3 (`:35-52`) reproduz clippy falhando; §11 (`:257-259`) lista 2 BLOCKERS; §8 marca #9/#22 e Anti-Slop #1/#6 como ❌. Só o cabeçalho foi editado.
5. **Benchmarks insuficientes para promoção** — detalhado em F3.
6. **Documentação afirma estado que não existe** — detalhado em F1/F2.

---

## F1 — MEMORY.md com estado stale

**Veredito: VIOLAÇÃO CONFIRMADA (mesma classe, conteúdo diferente do apontado)**

O implementador removeu durante esta auditoria os 3 itens originais que você apontou (`MEMORY.md:83-88`): "hook verificar-gate não implementado", "nenhum teste unitário escrito", "CI/CD não validado". Verificação: `MEMORY.md:85-87` agora só contém benchmarks executados + Task 0.1.5 adiada. **F1 como literalmente escrito foi parcialmente neutralizado no arquivo.**

Mas a classe de violação persiste e o documento continua **afirmando falsehoods verificáveis**:

| Linha | Afirmação | Realidade verificada |
|---|---|---|
| `MEMORY.md:5` | "`cargo clippy -D warnings` limpo" | **FALSO.** Exit 101, `clippy::len_zero` em `context_graph.rs:715` |
| `MEMORY.md:10` | "15/15 documentos" | **FALSO.** 14/15 — `IMPLEMENTATION_PLAN.md` ausente no repo (existe só em `cache/scratch/zylos-next-master-plan.md`, fora do repo). `README.md` da raiz também ausente |
| `MEMORY.md:4`, `:90` | "Próxima: Fase 3 (LIVE_COMM_SYSTEM)" | **CONTRADITÓRIO.** `TASKS.md:28` diz Fase 3 = VERIFICATION_PROOF; Master Plan: FASE 3 = VERIFICATION_PROOF, FASE 6 = LIVE_COMMUNICATION. Três numerações diferentes para a mesma fase |
| `MEMORY.md:94` | `[Master Plan v3.0](./IMPLEMENTATION_PLAN.md)` | **LINK MORTO.** Arquivo não existe no repo |
| `MEMORY.md:87` | Task 0.1.5 "adiada" | Conflita com `TASKS.md:4`, que a mantém em "Next" |

**Conceitos violados:** #7 (documentos vivos e atualizados a cada sessão), #17 (documentação que descreve um sistema que não existe é pior que ausência), Anti-Slop #1 (prova = artefato verificável).
**Nota a favor do implementador:** a estrutura de seções exigida pelo #7 está presente e completa (Estado Atual, Decisões Tomadas, Convenções de Processo, Lições Aprendidas, Abordagens que Falharam, Comandos Úteis, Issues Conhecidos, Próximo Passo Imediato, Referências).

## F2 — HANDOFF_0.md vs HANDOFF_1.md

**Veredito: F2 ORIGINAL RESOLVIDO — mas substituição por violação mais grave**

`HANDOFF_0.md:97-110` foi atualizado: "Status: COMPLETA — Fase 0 aprovada após dupla auditoria adversarial", checklist `:105-110` todo `[x]`, com referências às 4 auditorias. A inconsistência que você apontou não existe mais.

**Porém**, o documento que substituiu a fonte do problema é pior:

- `HANDOFF_2.md:56-58` "Dual Audit Reports" cita **dois** relatórios, um deles **inexistente** (`fase2-audit-v2.md`), ambos marcados APPROVED. `HANDOFF_2.md:3` declara "✅ APPROVED (Dual Audit Complete)" — mas **não há auditoria dupla**, há uma e meia.
- `HANDOFF_2.md:48-54` reporta benchmarks sob o título "Benchmark Evidence (`cargo bench -p zylos-memory-system --bench memory_system`)" com os números 189µs/1.55µs/73µs/39µs. Os números **conferem** com os artefatos do criterion (ver F3), mas o rótulo de evidência não pode coexistir com `HANDOFF_1.md:33-35`, que ainda diz "Não medido ainda" para as 3 métricas do bot-system.

**Conceitos violados:** #9 (gate de code review), #10 (auditoria isolada obrigatória — verificação de que as correções foram aplicadas), #17, Anti-Slop #1.

## F3 — Benchmarks não medidos

**Veredito: VIOLAÇÃO CONFIRMADA — escopo reduzido, blockers permanecem. Sozinho, já justifica CHANGES_REQUIRED.**

O que foi entregue nesta janela (verificado, não alegado):

- `crates/zylos-memory-system/benches/memory_system.rs` existe (169 linhas), `criterion` 0.5 em `Cargo.toml:25`, `[[bench]]` em `:27-29`.
- Artefatos reais em `target/criterion/`: `add_node` median **190.07µs** (slope 189.45µs), `get_node` **1.552µs**, `query_by_scope` **73.46µs**, `get_neighbors` **39.37µs**, `cosine_similarity/128` **0.8µs**.
- Os números citados em `MEMORY.md:8` e `HANDOFF_2.md:51-54` **batem** com os artefatos. Isso não é slop — é medição real.

O que ainda impede a promoção:

1. **7 métricas de `EVALUATION.md` (memory-system, linhas 19-28), só 2 medidas.** Faltam: VectorStore search <10ms, rerank <5ms, scope isolation overhead <0.1ms, **WAL checkpoint <50ms**, memory bloat 1M <500MB. Todos os 5 benches usam `ContextGraph::in_memory()` (`context_graph.rs:55-65`, `Connection::open_in_memory`) — **WAL nunca é exercitado**, logo o target de WAL é literalmente imensurável no harness atual.
2. **Métrica declarada é p95; o que foi medido é mediana.** `target/criterion/*/new/estimates.json` tem `mean`, `median`, `median_abs_dev`, `slope`, `std_dev` — **não tem campo p95**. O report compara mediana contra target "<1ms p95". No mérito o target se sustenta (pior amostra observada de `add_node` = 234.6µs, ~4× abaixo de 1ms), mas **o método declarado não foi provado**.
3. **Amostra insuficiente.** `sample.json` mostra `sampling_mode: "Linear"`, `iters [871, 1742, …, 8710]` → 10 amostras. p95 com n=10 não é estatisticamente defensável.
4. **As 3 métricas do bot-system nunca foram medidas.** `crates/zylos-bot-system/Cargo.toml:20-21` tem `dev-dependencies` = só `tokio-test 0.4`. **Zero** `criterion`, zero `[[bench]]`. Spawn <10ms p95, DAG <50ms p95, P2P <1ms p99 seguem sem medição. `HANDOFF_1.md:33-35` admite isso — e a Fase 1 foi declarada APPROVED assim mesmo.
5. **Infra de avaliação inteiro ausente:** `evals/datasets/` AUSENTE, `evals/baselines/` AUSENTE, `evals/judges/` 0 arquivos, `evals/benchmarks/` 0 arquivos, `evals/reports/` AUSENTE, `evals/run-evals.sh` AUSENTE — todos descritos em `EVALUATION.md:65-106`.
6. **Protocolo de promoção (`EVALUATION.md:92-97`) não cumprido:** exige benchmark verde + dataset versionado + painel adversarial + regressão ≤5% + sign-off. Só o 1º item existe, e incompleto.
7. **Benchmark não roda no CI.** `ci.yml` tem jobs check/fmt/clippy/test/coverage/security-audit/msrv/build-release — nenhum job de benchmark. Logo a regressão de performance não é detectável por máquina.
8. **`proptest` declarado e nunca usado.** `Cargo.toml` raiz declara proptest; `grep` em `crates/` e `apps/` = 0 usos. `AGENTS.md:49` diz "SEMPRE usar proptest para property-based testing em grafos/DAGs" — e o projeto tem `petgraph` em uso.

**Conceitos violados:** #22 ("Agente promovido sem benchmark = violação"), Anti-Slop #6 ("Nunca promova agente sem benchmark"), #7 (EVALUATION.md descreve um sistema de avaliação que não existe), Anti-Slop #1.

## F4 — TASKS.md stale

**Veredito: WARNING (isolado, não é blocker) — confirmado como stale, mas por motivos diferentes dos seus**

41 linhas não é, por si, defeito. O defeito é o conteúdo:

- `TASKS.md:5` e `TASKS.md:8` **duplicam** "Auditoria Dupla Fase 2" (Next **e** In Progress) enquanto a fase está declarada APPROVED em `HANDOFF_2.md:3` — o board não registra o estado real.
- `TASKS.md:11` "28/28 testes" → real **34/34**. `cargo test -p zylos-memory-system --lib -- --list` devolve: `context_graph` 8, `rerank` 11, `utils` 6, `vector_store` 9 = 34. (Task 2.1.1 = 8 ✓, Task 2.1.2 = 9 ✓, Task 2.1.3 = 10 ✗ → real 11, `utils` 6 nem é contabilizado.)
- `TASKS.md:4` mantém Task 0.1.5 em "Next"; `MEMORY.md:87` a declara adiada → conflito entre os dois documentos de estado.
- Numeração de fases divergente entre `MEMORY.md`, `TASKS.md` e Master Plan (ver F1).
- O Backlog foi limpo (só Fases 3-7) — isso **foi** corrigido.

**Conceitos violados:** #7, #17. Severidade: WARNING.

---

## Achados adicionais (não constavam da sua lista — verificados nesta auditoria)

### A1 — CRÍTICO — Zero commits, branch errada, CI jamais executável
`git log` → `fatal: your current branch 'master' does not have any commits yet`; `git for-each-ref` → 0 refs; `git branch -a` → vazio; sem remote; `git status --porcelain` → 26 entradas `??`. `.github/workflows/ci.yml:4-7` dispara em `branches: ["main"]` — o branch atual é `master`, sem commit. **O CI nunca rodou e não roda.** Viola #15 e Master Plan Task 0.1.6.

### A2 — CRÍTICO — Gates do CI vermelhos
`cargo clippy --workspace --all-targets -- -D warnings` → exit 101 (`clippy::len_zero`, `context_graph.rs:715`). `cargo fmt --all -- --check` → exit 1, 113 diffs em `crates/zylos-bot-system/src/dag.rs`, `lifecycle.rs`, `p2p.rs`, `crates/zylos-memory-system/src/context_graph.rs`, `rerank.rs`, `vector_store.rs`, `benches/memory_system.rs`. `cargo check --workspace --all-targets` → exit 0 (compila; quem falha é o lint, não o build).

### A3 — CRÍTICO — Gate de aprovação nunca dispara
`.git/hooks/` contém **apenas** `*.sample` — nenhum hook instalado. `scripts/verificar-gate.sh` (89 linhas, executável) só verifica presença da string `verify:` e extrai o comando com `grep 'cmd:' | head -1`; **não detecta stubs, TODOs ou ausência de código**. Master Plan Task 0.1.6 exige explicitamente "bloqueia stubs/TODOs abertos". Único log de execução em `evals/audits/GOAL-CONTRACT-verify-20261029-211946.log`: `bash: -c: line 1: syntax error near unexpected token 'newline'` + `Expected exit: 0 / Actual exit: 2` → o hook executou o **placeholder** `<comando shell que prova o resultado>` do template `specs/GOAL-CONTRACT.md` e **falhou**, e o projeto avançou assim mesmo. Viola #9 e Anti-Slop #1.

### A4 — CRÍTICO — Critérios de saída do Master Plan não cumpridos
FASE 1 exige "Benchmarks Memanto + LongMemEval + MARBLE passam" + "Migration adapter funcional (one-way)" → **0 hits** para `memanto|longmemeval|marble|migration_adapter` em `crates/` + `apps/`. FASE 2 exige "Workflows 100+ nós com rollback automático" + "RSI protocol funcional e auditável" → **0 hits** para `shadow_git|recovery_ladder|agent_track|workflow_dsl`. **6 de 8 crates são stub de 1 linha** (`zylos-core`, `zylos-live-comm`, `zylos-mobile`, `zylos-sandbox`, `zylos-skills`, `zylos-verification`) e os 3 apps têm 5 linhas cada (`zylos-cli`, `zylos-daemon`, `zylos-gateway`). O projeto também **inverteu as fases** do Master Plan (plano: F1=MEMORY, F2=BOT; projeto: F1=BOT, F2=MEMORY) sem registrar a decisão em `DECISIONS.md`.

### A5 — CRÍTICO — Documentos obrigatórios ausentes
Da tabela "Arquivos Obrigatórios por Projeto" do Manifesto (#19): **14/15 presentes**; falta `IMPLEMENTATION_PLAN.md` (o Master Plan existe apenas em `cache/scratch/`, fora do repo, e `MEMORY.md:94` aponta para link morto). Falta também `README.md` na raiz (#17) e **0 READMEs** em `crates/`/`apps/` (`AGENTS.md:56`). `docs/`, `skills/`, `examples/`, `tests/` vazios. `specs/HERMES-INTEGRATION.md` (Task 0.1.5) ausente.

### A6 — WARNING — Cobertura nunca medida
`TESTING.md` tem **25 marcadores ☐ e 0 marcados**; `TESTING.md:94-98` exige >80% line coverage, 100% de funções públicas, todos os error paths e benchmark obrigatório. `cargo llvm-cov` **não está instalado** (`error: no such command: llvm-cov`); `cargo audit` também não. `HANDOFF_1.md:32` reporta ">80% coverage | 21/21 passando" — confunde contagem de testes com cobertura. `ci.yml` tem job `coverage` (linha 53) que instala `cargo-llvm-cov`, mas com zero commits nunca executou.

### A7 — WARNING — Segurança: hash MD5 e erros silenciados
- `crates/zylos-memory-system/src/types.rs:111` usa `md5::compute` para content hash; `Cargo.toml:19` declara `md5 = "0.7"`; `sha2` ausente.
- `unwrap_or_default()` em `context_graph.rs:389`, `rerank.rs:224/413/433`, `types.rs:110` — engole erro e devolve valor plausível. Anti-Slop #3 (nunca pule estado de erro).
- `crates/zylos-memory-system/src/rerank.rs:270-273` — `scope_matches` retorna `true` sempre, com comentário "For now, all scopes match". É stub que passa em teste.
- `0` marcadores TODO/FIXME/HACK em `crates/`+`apps/` — não porque não existam, mas porque o gate que deveria detectá-los nunca roda (A3).

### A8 — INFO — Divergências de ambiente
`rust-toolchain.toml` fixa `stable` + `wasm32-unknown-unknown`; `ci.yml` tem job `msrv` com 1.82 (`rust-version = "1.82"` no workspace) — 1.82 não está instalado localmente, logo o MSRV não foi verificado. `MEMORY.md:11` afirma "sqlite3 3.47.0 instalado" — **verdadeiro** (existe em `C:/Users/Paulo/AppData/Local/Programs/sqlite3/sqlite3.exe`, 3.47.0), apenas não está no PATH do shell MSYS. `wasm-pack 0.15.0` e `wasmtime 50.0.0` confirmados.

---

## Por que as auditorias v1 e v2 perderam estes pontos

Não foi (só) família de modelo. A evidência é mecânica: **nenhum dos 5 relatórios existentes executou os gates que o próprio CI define.**

| Relatório | `cargo fmt` | `cargo clippy` | `git log`/`git status` |
|---|---|---|---|
| `fase0-fase1-audit.md` | 0 | 0 | 0 |
| `fase0-fase1-audit-v2.md` | 0 | 0 | 0 |
| `fase0-fase1-audit-v3-glm.md` | 0 | 3 | 0 |
| `fase0-fase1-audit-v3-minimax.md` | 0 | 0 | 0 |
| `fase2-audit-adversarial-minimax.md` | 0 | 2 | 0 |

O v2 (APPROVED) rodou `cargo test -p zylos-bot-system --lib` e `cargo check --workspace --all-targets` — **nenhum dos dois pega lint, formatação ou estado do git**. O v3-glm rodou clippy e ainda assim aprovou porque rodou `cargo check`/clippy **antes** das correções finais e não re-executou depois.

**Causa raiz real:** o protocolo de auditoria não exige a lista de comandos. Faltou um checklist mandatório: (a) rodar os jobs do CI localmente, (b) verificar `git log`/branch/remote, (c) conferir que todo arquivo referenciado em documento existe, (d) conferir que todo `verify:<cmd>` executa. Auditoria por leitura de Markdown não encontra exit code 101.

Mitigação de viés de família: exigir que o auditor nunca valide documentos contra si mesmos, e que o painel de julgamento receba **stdout do comando**, não afirmação do implementador. Adicionalmente, os números de benchmark publicados devem ser extraídos de `target/criterion/*/new/estimates.json` (fonte primária), nunca redigitados de tabela em Markdown.

---

## Correções exigidas antes de nova submissão

**Blockers (CHANGES_REQUIRED — Fase 2 não fecha sem todos):**

1. `git init` já existe → criar branch `main`, **commit** o estado, configurar remote. Push com CI verde. (#15, Task 0.1.6)
2. `cargo fmt --all` → `cargo fmt --all -- --check` exit 0.
3. `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (hoje: `context_graph.rs:715` `len_zero`).
4. `cargo test --workspace --all-features` verde **e** `cargo llvm-cov --workspace --all-features -- --fail-under-lines 80` verde. Instalar `cargo-llvm-cov` e `cargo-audit`.
5. Instalar hooks de `scripts/verificar-gate.sh` em `.git/hooks/` **e** estendê-lo para bloquear stubs/TODOs (não só presença de `verify:`). Corrigir `specs/GOAL-CONTRACT.md` para conter comando real, e re-executar o gate com exit 0 comprovado.
6. Criar `evals/audits/fase2-audit.md` e `evals/audits/fase2-audit-v2.md` **de verdade**, ambos APPROVED, com auditor isolado; referenciar os dois em `HANDOFF_2.md:56-58`. Corrigir o relatório contraditório (`fase2-audit-adversarial-minimax.md:303-309`) ou removê-lo.
7. Benches em `zylos-bot-system` (`criterion` + `[[bench]]`) cobrindo spawn, DAG linear e entrega P2P. Criterion não emite p95 → reportar p95 calculado explicitamente ou mudar o target em `EVALUATION.md` para a estatística que a ferramenta de fato produz (e documentar a mudança). Ampliar `--sample-size` (mín. 100) para sustentar estatística p95.
8. Medir as 5 métricas restantes de memory-system (`EVALUATION.md:19-28`), incluindo **WAL** — o harness atual só faz `in_memory()`. Criar `evals/datasets/`, `evals/baselines/v1.0/`, `evals/judges/`, `evals/run-evals.sh`, `evals/reports/`.
9. Job de benchmark no `ci.yml` (regressão de performance automatizada) + job de proptest.
10. `IMPLEMENTATION_PLAN.md` dentro do repo (link `MEMORY.md:94` volta a funcionar), `README.md` na raiz e README por crate/app (`AGENTS.md:56`).
11. `MEMORY.md` corrigido: linha 5 (clippy), linha 10 (contagem real), linhas 4/90 (numeração de fases — alinhar com `TASKS.md` e Master Plan), linha 94 (link), linha 87 (Task 0.1.5). `HANDOFF_1.md:32` corrigir (não é coverage). `HANDOFF_2.md:56-58` corrigir.
12. `TASKS.md` corrigido: linhas 5/8 (duplicata de auditoria), 11 (28→34), 12 (10→11 testes), 4 (Task 0.1.5) e numeração de fases.
13. Preencher os 25 checkboxes de `TESTING.md` com o estado real.
14. Registrar em `DECISIONS.md` a inversão de fases vs Master Plan e a decisão de escopo.
15. Justificar ou remover os stubs de 6 crates e os 3 apps de 5 linhas; ou declarar o escopo real no `PRD.md` Out of Scope (#20).
16. MD5 → SHA-256 em `types.rs:111`; eliminar `unwrap_or_default` em favor de propagação de erro; implementar `scope_matches` ou marcar como stub explícito no escopo.
17. `evals/audits/` deve conter **somente** relatórios reais e consistentes; toda referência em documento precisa resolver para arquivo existente.

**Warnings (não bloqueiam, mas são exigidos na próxima revisão):** `TESTING.md` incompleto, MSRV 1.82 não verificado, `sqlite3` fora do PATH do shell.

---

## Evidência de execução (comandos e resultados)

```
cargo clippy --workspace --all-targets -- -D warnings   → exit 101
  error: length comparison to one
     --> crates/zylos-memory-system/src/context_graph.rs:715:17
  715 |         assert!(results.len() >= 1);
      = note: `-D clippy::len-zero` implied by `-D warnings`

cargo fmt --all -- --check                              → exit 1  (113 "Diff in")

cargo check --workspace --all-targets                   → exit 0

cargo test --workspace --all-features
  zylos-bot-system      → 21 passed; 0 failed
  zylos-memory-system  → 34 passed; 0 failed
  (demais 18 crates/apps → 0 tests)

git log        → fatal: your current branch 'master' does not have any commits yet
git for-each-ref → (vazio)     git status --porcelain | wc -l → 26  (todos "??")
git remote -v  → (vazio)

ls evals/audits/ → fase0-fase1-audit.md, fase0-fase1-audit-v2.md,
                   fase0-fase1-audit-v3-glm.md, fase0-fase1-audit-v3-minimax.md,
                   fase2-audit-adversarial-minimax.md, GOAL-CONTRACT-verify-20261029-211946.log
                   → fase2-audit-v2.md AUSENTE (citado em HANDOFF_2.md:58)

grep -c "cargo fmt" em evals/audits/*.md      → 0 0 0 0 0
grep -ci "git log\|git status" nos 5 .md      → 0 0 0 0 0

target/criterion/*/new/estimates.json
  add_node/single          median 190071.024 ns  (CI95 179069–211121)   slope 189449 ns
  get_node/lookup_by_id    median   1552.3 ns
  query_by_scope           median  73462.7 ns
  get_neighbors/hub_20     median  39372.9 ns
  cosine_similarity/128    median      0.8 µs
  → nenhum campo p95 em estimates.json; sample.json: n=10, sampling_mode Linear

grep -rniE "memanto|longmemeval|marble|migration_adapter|shadow_git|recovery_ladder|agent_track|workflow_dsl" crates apps → 0
cargo llvm-cov --version → error: no such command: `llvm-cov`
cargo audit --version   → error: no such command: `audit`
```

**Nota de método:** o repositório estava sendo modificado por um agente implementador durante toda a auditoria (mtimes entre 03:19 e 04:21). Todos os vereditos acima referem-se ao snapshot de 2026-10-30 06:18 (-03:00), com hashes SHA-256 listados no cabeçalho. Qualquer re-submissão deve ser auditada contra um **commit** — não contra uma árvore de trabalho mutável.

---

**Assinatura**

**Auditor:** Auditoria Adversarial Independente v4 (instância isolada)
**Veredito:** ❌ CHANGES_REQUIRED
**Conceitos violados:** #6, #7, #9, #10, #15, #17, #19, #20, #22
**Anti-Slop violados:** #1, #3, #6
**Blockers:** 16 (A1-A5 + F3 como benchmark incompleto)
**Warnings:** 6
**Próxima ação:** corrigir os 16 blockers, commitar na `main`, e re-submeter para auditoria isolada de verificação (`fase2-audit-v2.md`) com **stdout dos comandos** anexado.
