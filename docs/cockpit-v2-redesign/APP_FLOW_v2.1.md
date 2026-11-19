# APP_FLOW: Zylos Cockpit v2.1

**Versão:** 2.1.0 (proposta, substitui a arquitetura de telas de [APP_FLOW.md](./APP_FLOW.md) v2.0)
**Data:** 2026-11-19
**Status:** Implementado (ver §14 para o que ficou de fora)
**Par:** [DESIGN_SYSTEM_v2.1.md](./DESIGN_SYSTEM_v2.1.md) · [mockup-v2.1.html](./mockup-v2.1.html)

---

## 1. Princípio de arquitetura de informação

A v2.0 tinha 11 telas e uma sidebar de 4 grupos; o código tem mais de 40 painéis. A pessoa precisa responder três perguntas, e cada uma ganha uma aba:

| Pergunta | Aba | O que contém |
|---|---|---|
| "O que estou pedindo aos agentes?" | **Chat** | Conversa por Space e Thread, cartões de aprovação inline, computador do agente inline |
| "O que está esperando por mim?" | **Inbox** | Aprovações, pedidos de senha, perguntas de agentes |
| "O que aconteceu?" | **Activity** | Trilha, recibos, "Resolvido por você e pelos agentes", alertas do Sentinela |

O resto (agentes, memória, metas, DAG, personalidades, configurações) é **contexto**, não navegação principal: vive no Rail, no menu do avatar ou no palette.

## 2. Mapa de telas (v2.0 para v2.1)

| v2.0 | Rota v2.0 | v2.1 | Onde fica |
|---|---|---|---|
| AF-01 Agents Grid | `/agents` | **Roster de agentes** | Rail (lista) + folha "Agentes" com busca; sem grade como tela inicial |
| AF-02 Tasks + Approvals | `/tasks` | **Inbox** + tarefas dentro do Chat | Aba Inbox; tarefas como linhas na Thread |
| AF-03 Memory Graph | `/memory` | **Vista Memória** | Rail, abre na coluna principal |
| AF-04 Goals Cascade | `/goals` | **Vista Metas** | Rail |
| AF-05 DAG Swarm | `/dag` | **Vista DAG** | Rail, também aberta pelo palette |
| AF-06 Sentinel AIOps | `/sentinel` | **Activity** (filtro "Alertas") | Aba Activity |
| AF-07 Phone Relay (VNC) | `/phone-relay` | **Painel Computador** | Inline no Chat e painel lateral |
| AF-08 Personalities | `/personalities` | Configurações > Agentes | Menu do avatar |
| AF-09 Settings | `/settings` | Configurações | Menu do avatar |
| AF-10 Command Palette | overlay | mantido, sem animação | Global |
| AF-11 Toast | overlay | mantido | Global |

Novas rotas propostas (as antigas continuam respondendo com redirecionamento 302 para não quebrar links):

| Rota | Tela |
|---|---|
| `/` | Chat do último Space |
| `/chat/:space/:thread?` | Chat |
| `/inbox` | Inbox |
| `/activity` | Activity |
| `/space/:id/memory`, `/space/:id/goals`, `/space/:id/dag` | Vistas do Space |
| `/computer/:session` | Painel Computador (também abre inline) |
| `/settings/:secao` | Configurações |

**Spaces:** agrupam conversas, memória, metas e quais agentes têm acesso (como em OpenDots, o acesso é explícito). Para a primeira entrega, um Space "Geral" cobre tudo.

## 3. Shell (persistente)

### Entrada
Qualquer rota monta: cabeçalho flutuante, Rail (estado de colapso do localStorage), assinatura SSE, contêiner de toasts.

### Estados do shell
| Estado | Comportamento |
|---|---|
| Carregando | Rail e cabeçalho em esqueleto com a forma final; sem salto de layout |
| Vazio | Não se aplica |
| Erro | SSE caiu: faixa discreta "Reconectando" no cabeçalho + toast após 3 falhas; novas tentativas 1s, 2s, 4s até 30s |
| Sucesso | Shell completo |
| Validação | Não se aplica |
| Desabilitado | Abas continuam navegáveis offline com dados em cache; ações que mudam estado ficam desabilitadas com explicação ("Sem conexão") |

### Recuperação
Reconexão com recuperação por REST ao voltar o foco (mantido da v2.0). Sessão expirada leva à tela de login (futuro).

## 4. Chat (`/chat/:space/:thread`)

### Ações
1. Escrever no composer e enviar (`Enter`; `Shift+Enter` quebra linha).
2. Mencionar agente com `@`.
3. Ver e expandir chamadas de ferramenta (recolhidas por padrão).
4. Decidir cartões de aprovação inline.
5. Expandir a miniatura do computador do agente.
6. Trocar de Thread ou Space no Rail.

### Respostas do sistema
- Envio: mensagem aparece na hora (otimista), agente responde em fluxo (streaming de texto).
- Aprovação pedida: o cartão entra na Thread **e** gera linha no Inbox com contador.
- Computador em uso: miniatura aparece no ponto da conversa onde começou.

### Estados
| Estado | Renderização |
|---|---|
| Carregando | Esqueleto de 3 mensagens com a forma real |
| Vazio | Texto curto "Peça algo a um agente" + 3 sugestões clicáveis relevantes ao Space |
| Erro | Mensagem não enviada fica marcada com `Tentar de novo`; erro de stream mostra "Resposta interrompida" + `Continuar` |
| Sucesso | Thread renderizada |
| Validação | Composer: aviso inline se a mensagem exceder o limite; anexo inválido explica o motivo |
| Desabilitado | Composer desabilitado com motivo ("Agente offline", "Sem conexão") |

## 5. Aprovação inline (ADR-001 revisado)

Três ações: **Recusar**, **Permitir uma vez**, **Sempre permitir** (com regras de §5.2).

### 5.1 Fluxo
```
1. SSE approval_requested  -> cartão monta na Thread (200ms, opacidade + 8px) e linha no Inbox (+1)
2. Pessoa lê: ação, agente, risco, hash
3a. Permitir uma vez -> botão mostra progresso, POST /api/v1/approvals/:id {decision:"allow_once"}
3b. Recusar          -> POST {decision:"refuse"}; opcional motivo curto (campo inline, não modal)
3c. Sempre permitir  -> confirmação inline com o escopo exato -> POST {decision:"allow_always", scope}
4. Sucesso: SSE approval_resolved -> cartão colapsa em recibo de 1 linha; Inbox -1; entra em Activity
5. Falha: reverte o otimismo, mostra erro no próprio cartão + Tentar de novo
6. Recibo persistido em localStorage (TTL 24h); recarregar restaura
```

### 5.2 Regras de "Sempre permitir" (proposta, depende de D1 e D2 do DESIGN_SYSTEM)
- Desabilitado para risco ALTO, com explicação visível.
- Escopo fixo: agente + tipo de ação + Space; mostrado por extenso antes de confirmar.
- Gera regra revogável em Configurações > Permissões, com data e quem criou.
- Toda decisão registra hash, usuário e horário na trilha (mantém o rastro de auditoria da v2.0).

### 5.3 Estados do cartão
| Estado | Renderização |
|---|---|
| Carregando | Esqueleto do cartão enquanto carrega detalhes |
| Vazio | Não se aplica |
| Erro | Erro inline no cartão + `Tentar de novo`; toast só se a falha for de rede |
| Sucesso | Recibo colapsado |
| Validação | Hash divergente: aviso em `--err`, botões de permitir desabilitados |
| Desabilitado | Pós-decisão travado como recibo; `Sempre permitir` desabilitado conforme §5.2 |

### 5.4 Recuperação
POST falhou: reverte e permite tentar. Evento SSE perdido: sincronização por REST ao voltar o foco. Recibo velho: limpeza automática após 24h.

## 6. Inbox (`/inbox`)

### Ações
1. Ver pendências agrupadas por agente, mais antigas primeiro.
2. Decidir direto na linha (mesmos 3 botões nas aprovações).
3. Responder pergunta de agente em campo inline.
4. Abrir o contexto no Chat (leva ao ponto da Thread).
5. Filtrar por agente, Space e tipo.

### Estados
| Estado | Renderização |
|---|---|
| Carregando | 5 linhas em esqueleto |
| Vazio | "Nada esperando por você" + atalho para Activity (estado positivo, sem ilustração pesada) |
| Erro | Faixa "Falha ao carregar" + `Tentar de novo`; itens em cache seguem visíveis |
| Sucesso | Lista de linhas |
| Validação | Resposta vazia desabilita o envio com dica |
| Desabilitado | Linhas decididas ficam travadas enquanto o POST corre |

O contador do cabeçalho some em zero. Notificação do navegador para pendência de risco ALTO (com permissão).

## 7. Activity (`/activity`)

### Conteúdo
- **Linha do tempo** de recibos, execuções, alertas do Sentinela e eventos de agentes.
- **"Resolvido por você e pelos agentes":** o que correu sem precisar perguntar; cada item mostra o que foi feito e `Desfazer` quando houver desfazer real.
- Filtros: agente, Space, tipo (Aprovações, Execuções, Alertas).

### Estados
| Estado | Renderização |
|---|---|
| Carregando | Esqueleto de linhas |
| Vazio | "Sem atividade ainda" + explicação de que o que os agentes fazem aparece aqui |
| Erro | Faixa + `Tentar de novo` |
| Sucesso | Linha do tempo paginada (carregar mais ao final) |
| Validação | Intervalo de datas inválido: aviso inline |
| Desabilitado | `Desfazer` desabilitado quando não é mais possível, com motivo |

## 8. Painel Computador (substitui Phone Relay, ADR-003 mantido)

### Entrada
Miniatura no Chat, `/computer/:session`, ou palette "Abrir computador".

### Fluxo
```
1. Montar: verificar dispositivos pareados
2a. Nenhum -> estado vazio com QR de pareamento
2b. Existe -> WS /ws/computer-view/{session_id}
3. WS aberto -> quadro preto + "Conectando" (esqueleto com a forma do quadro)
4a. Quadro em até 3s -> ao vivo
4b. Sem quadro em 3s -> screenshot de reserva + `Tentar de novo`
5. Monitor de qualidade: média de intervalo > 66ms reduz resolução em 50%
6. Assumir controle -> POST /api/v1/computer-view/:session/lease [CRIAR ENDPOINT, pendência herdada da v2.0]
7. Lease expira em 5 min ou Liberar -> DELETE lease; volta a "Visualizando"
8. WS fecha ou erra -> fallback automático para screenshot + toast
```

### Estados
| Estado | Renderização |
|---|---|
| Carregando | "Conectando" sobre o quadro em esqueleto |
| Vazio | "Nenhum dispositivo pareado" + QR (com contagem de expiração e renovação) |
| Erro | Screenshot de reserva + `Tentar de novo` + toast |
| Sucesso | Stream ao vivo + controles |
| Validação | Não se aplica |
| Desabilitado | Camada "Controle liberado" + `Assumir controle` quando o lease expira |

## 9. Vistas de Space (Memória, Metas, DAG)

Mantêm o conteúdo funcional da v2.0 (AF-03, AF-04, AF-05) dentro da coluna principal, com o mesmo cabeçalho e Rail. A tabela de estados da v2.0 vale sem mudança, trocando apenas as ilustrações pesadas por texto curto e uma ação.

| Vista | Carregando | Vazio | Erro |
|---|---|---|---|
| Memória | Esqueleto do grafo | "Memória vazia" + nota sobre consolidação noturna | Falha + retry |
| Metas | Esqueleto de 3 níveis | "Nenhuma meta definida" + ação | Falha + retry |
| DAG | Esqueleto + "Renderizando" | "Swarm inativo" | Falha + retry |

## 10. Command palette

### Entrada
`⌘K` / `Ctrl+K`; botão de busca no cabeçalho.

### Comportamento
- Abre e fecha **instantaneamente** (sem animação).
- Busca com debounce de 50ms, grupos: Recentes, Páginas, Agentes, Spaces, Comandos.
- Comandos: Abrir Inbox, Abrir Activity, Abrir computador, Novo Space, Alternar tema, Alternar Rail.
- Setas navegam, `Enter` seleciona, `Esc` fecha.

### Estados
| Estado | Renderização |
|---|---|
| Carregando | Indicador discreto no campo durante o debounce |
| Vazio | "Nenhum resultado" + 3 sugestões |
| Erro | Toast se a ação falhar após selecionar |
| Sucesso | Resultados agrupados com seleção destacada |
| Validação | Atalho inválido no contexto: aviso curto |
| Desabilitado | Campo desabilitado durante a execução |

## 11. Toast

Mantido da v2.0 com as mudanças de [DESIGN_SYSTEM_v2.1.md §6.8](./DESIGN_SYSTEM_v2.1.md): variante por ícone e texto, sem faixa lateral, transição CSS interrompível, pausa no hover e com aba oculta, clique leva à entidade relacionada.

## 12. Jornadas principais

| Jornada | Passos |
|---|---|
| Aprovar uma ação | Notificação ou contador do Inbox, lê o cartão, `Permitir uma vez`, recibo aparece, volta ao que fazia (menos de 3 passos) |
| Pedir algo a um agente | `Ctrl+K` ou foco no composer, escreve, acompanha o stream, aprova se pedido |
| Assumir um computador | Miniatura no chat, expande, `Assumir controle`, libera ao terminar |
| Revisar o dia | Activity, "Resolvido por você e pelos agentes", `Desfazer` se preciso |
| Celular à noite | Barra de abas inferior, Inbox, decide, tema escuro automático |

## 13. Matriz de rastreio

| Fluxo v2.1 | Componente (DESIGN_SYSTEM_v2.1) | Origem (v2.0) |
|---|---|---|
| Chat | Mensagem §6.2, Avatar §6.3 | AF-02 parcial |
| Aprovação | Cartão §6.4 | AF-02, ADR-001 |
| Inbox / Activity | Linhas §6.5 | AF-02, AF-06 |
| Computador | Painel §6.6 | AF-07, ADR-003 |
| Palette | §6.7 | AF-10, ADR-009 |
| Toast | §6.8 | AF-11 |
| Vistas de Space | §5 | AF-03, AF-04, AF-05 |

## 14. Estado da implementação (2026-11-19)

| Item | Situação |
|---|---|
| Abas Chat / Inbox / Activity | **Feitas**, com barra inferior no celular |
| Aprovação com 3 ações, regras permanentes e revogação | **Feitas** (rotas `POST /approvals`, `POST /approvals/resolve`, `GET /approvals/rules`, `POST /approvals/rules/revoke`) |
| Eventos SSE de aprovação e alerta | **Feitos** no gateway (`ApprovalRequested`, `ApprovalResolved`, `ApprovalTimeout`, `SentinelAlert`) |
| Painel Computador inline | **Feito** como cartão no chat; WS e lease funcionam. Frames reais e latência não testados |
| Endpoint de lease | **Já existia** (`/computer-view/sessions/:id/control`), mas as rotas `{id}` do axum 0.7 respondiam 404; corrigido |
| Spaces e Rail de Spaces | **Não feitos** (precisam de modelo de dados e API) |
| "Resolvido por você e pelos agentes" com Desfazer | **Não feito** (o backend não registra ações autônomas dos agentes) |
| Redirecionamento 302 das rotas antigas | **Não feito**: o cockpit navega por estado, não por rotas de URL |
| Mapeamento dos 40+ componentes às telas novas | Parcial: as 34 abas antigas continuam na sidebar |

## 14.1 Bugs do backend encontrados ao implementar (corrigidos)

1. `POST /api/v1/approval/resolve` (singular) usa outro store e outro enum; resolver pelo cockpit dava 404/422. A resolução ficou em `/approvals/resolve`.
2. Gateway não migrava o SQLite de aprovações: banco novo respondia 500.
3. `/approvals/resolve` ficou de fora de `rota_admin` (qualquer papel aprovava).
4. O cockpit enviava a decisão `escalate`, que o backend recusa com 400.
5. Rotas `{id}` (sintaxe do axum 0.8) no axum 0.7: sessão, screenshot, lease e `/goals/:id` sempre 404.
6. WebSocket do computador pedido em `/ws/...` quando o gateway monta em `/api/v1/ws/...`, e sem autenticação por `?access_token=`.
7. `trunk build` falhava: o hook do wasm-opt recebia `$WASM_FILE` literal.

## 15. Changelog

| Versão | Data | Mudança |
|---|---|---|
| 2.1.0 | 2026-11-19 | Arquitetura de 3 abas, aprovação em 3 ações, computador inline, Spaces |
