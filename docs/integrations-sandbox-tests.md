# Testes de Integração Sandbox — Slack / Notion / Linear (T11.8.7)

## Objetivo
Validar os adapters de `crates/zylos-integrations` contra (a) um servidor HTTP mock local
e (b) as APIs reais das plataformas, sem efeitos colaterais.

## Arquivos
| Arquivo | Conteúdo |
|---|---|
| `crates/zylos-integrations/tests/common/mod.rs` | `MockServer` (axum em `127.0.0.1:0`): rotas JSON canned, captura método/path/query/Authorization/Notion-Version/body |
| `crates/zylos-integrations/tests/integration_slack.rs` | 6 mock + 2 sandbox |
| `crates/zylos-integrations/tests/integration_notion.rs` | 5 mock + 2 sandbox |
| `crates/zylos-integrations/tests/integration_linear.rs` | 5 mock + 2 sandbox |
| `.github/workflows/integrations-sandbox.yml` | Job CI (workflow_dispatch + schedule semanal + push em paths das integrações) |

## Dois níveis de teste
1. **`mock_*` (sempre rodam)** — adapters criados com `with_base_url(...)` apontando para o mock.
   Verificam headers de auth (Slack/Notion `Bearer`, Linear PAT cru), `Notion-Version: 2022-06-28`,
   payloads (`chat.postMessage`, `/blocks`, `commentCreate`), parsing e propagação de erros.
2. **`sandbox_*` (`#[ignore]`)** — APIs reais, **somente leitura**:
   - Slack: `auth.test`, `conversations.list` (aceita `missing_scope` como skip parcial)
   - Notion: `GET /users/me`, `POST /search` (page_size=1)
   - Linear: query `viewer`, query `teams`
   Sem a variável de token, o teste imprime `SKIP: <VAR> ausente` e passa.
   Nenhum teste publica mensagens, cria páginas ou issues.

## Como rodar
```bash
export CARGO_TARGET_DIR=E:/zylos-target/integrations
cargo test -p zylos-integrations                         # unit + mock
SLACK_BOT_TOKEN=... NOTION_TOKEN=... LINEAR_TOKEN=... \
  cargo test -p zylos-integrations --test integration_slack \
  --test integration_notion --test integration_linear -- --ignored --nocapture
```

## CI
O workflow `Integrations Sandbox` injeta os secrets `SLACK_BOT_TOKEN`, `NOTION_TOKEN`,
`LINEAR_TOKEN` do repositório e imprime apenas **presente/ausente** (nunca o valor).
`workflow_dispatch` só aparece após o workflow existir na branch default; até lá ele roda via
`push` nos paths das integrações.

## Segurança
- Tokens nunca são logados; o mock registra o header apenas em memória do processo de teste.
- Chamadas sandbox são idempotentes e read-only.
