# Painel de Integrações do Cockpit (T11.8.6)

## Visão geral
Aba **Integrações** no cockpit (`Tab::Integrations`) renderizando
`apps/zylos-cockpit/src/components/integrations_panel.rs`. Mostra, por integração
(Slack, Notion, Linear): status configurado/não configurado, variável de token esperada,
token **mascarado** (marcador fixo `••••••••`), indicação de URL sandbox customizada,
link do playbook de setup e botão **Testar conexão**.

## Endpoints do gateway (`apps/zylos-gateway/src/integrations_api.rs`)
| Método | Path | Resposta |
|---|---|---|
| GET | `/api/v1/integrations/status` | `{ "integrations": [ { name, display_name, configured, token_env, token_masked, custom_base_url, docs } ] }` |
| POST | `/api/v1/integrations/{name}/test` | `{ name, configured, ok, latency_ms, detail }` — `404` para nome desconhecido |

`test` chama `ChannelAdapter::health_check()` (read-only: Slack `auth.test`, Notion `/users/me`,
Linear `viewer`).

## Configuração (variáveis de ambiente do gateway)
| Integração | Token | URL customizada (sandbox/mock, opcional) |
|---|---|---|
| Slack | `SLACK_BOT_TOKEN` | `ZYLOS_SLACK_API_URL` |
| Notion | `NOTION_TOKEN` | `ZYLOS_NOTION_API_URL` |
| Linear | `LINEAR_TOKEN` | `ZYLOS_LINEAR_API_URL` |

Valores em branco contam como não configurado. As variáveis são lidas a cada requisição —
basta reiniciar/reconfigurar o gateway e clicar **Recarregar**.

## Regras de segurança
- O valor do token **nunca** sai do gateway: nem prefixo, sufixo ou tamanho. Teste
  `status_lists_three_and_never_leaks_token` garante que nem `xoxb-` aparece na resposta.
- O painel exibe o campo de token como `readonly`; não há entrada de segredo pelo browser
  (fora de escopo: armazenamento de tokens via UI/OAuth — usar playbooks
  `pb-*-integration-setup.md`).

## Testes
- Gateway: 6 testes em `integrations_api::tests` (status, máscara, branco, 404, sem token,
  mock OK, mock invalid_auth).
- Cockpit: 4 testes unitários de helpers (`default_integrations`, `token_display`,
  parse JSON, `format_test_result`).
