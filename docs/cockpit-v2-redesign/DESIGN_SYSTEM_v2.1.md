# Design System: Zylos Cockpit v2.1

**Versão:** 2.1.0 (proposta, substitui a direção visual de [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) v2.0)
**Data:** 2026-11-19
**Status:** Implementado no cockpit (tokens, tema, fontes, ícones, navegação, cartão de aprovação, computador inline). Ver §11 para o que ficou de fora.
**Par:** [APP_FLOW_v2.1.md](./APP_FLOW_v2.1.md) · mockup: [mockup-v2.1.html](./mockup-v2.1.html)
**Fonte única de tokens (mantida de ADR-008):** `apps/zylos-cockpit/styles/design-tokens.css`

---

## 0. Por que existe a v2.1

A auditoria da v2.0 achou 10 problemas (ver resumo em §12). Os três que mudam a direção:

1. O visual era o default de IA: slate escuro `#0f172a`, azul `#3b82f6`, Inter, vidro em tudo, grade de cards iguais.
2. A navegação era uma lista de 11 telas e mais de 40 painéis. Os repositórios de referência têm 3 abas.
3. O cartão de aprovação usava faixa lateral colorida e botões que não distinguem "só desta vez" de "sempre".

Referências lidas: OpenDuet (cabeçalho flutuante, abas Chat / Inbox / Activity, cartão Refuse / Allow once / Always allow, "Handled for you"), OpenDots (avatares-personagem por agente, Spaces e Threads, computador do agente dentro do chat, tema System / Light / Dark) e Rerun (figura central "Chief of Staff", quadro mobile). Não copiamos pixels nem assets: copiamos a estrutura.

## 1. Leitura do projeto

> Painel de operação de uma pessoa que supervisiona agentes de IA, centrado em conversa, com linguagem calma e legível, apoiado em tokens CSS próprios (Rust/Leptos, sem Tailwind).

- **Registro:** produto (o design serve à tarefa), não marca.
- **Cena de uso (decide claro vs escuro):** uma pessoa sozinha no escritório, de dia, lendo e aprovando ações de agentes em um monitor; à noite, a mesma pessoa em quarto escuro, no celular, aprovando algo urgente. Dois ambientes reais, então **claro e escuro, seguindo o sistema**, com troca manual. Claro é o padrão visível nas capturas e na documentação.
- **Dials (taste-skill):** variância 4, movimento 3, densidade 5. Produto de uso diário: pouca coreografia, densidade média.
- **Estratégia de cor (impeccable):** Restrained. Neutros tingidos de azul-frio, ação primária em tinta (quase preto), **um único destaque "sinal"** usado só onde o humano precisa agir.

## 2. Cor

Todas as cores em OKLCH, definidas só em `design-tokens.css`. Sem `#000`, sem `#fff`.
Contrastes abaixo foram **calculados** (WCAG 2.x, script de cálculo guardado fora do repositório; refazer com `oklch` → sRGB antes de mudar qualquer token).

### 2.1 Claro (padrão)

| Token | OKLCH | Hex aprox. | Uso | Contraste |
|---|---|---|---|---|
| `--paper` | `98.6% 0.004 260` | `#f9fafd` | Fundo do app | base |
| `--surface` | `99.6% 0.002 260` | `#fdfeff` | Cartões, composer, popovers | base |
| `--sunken` | `96.2% 0.006 260` | `#f0f3f7` | Rail, áreas rebaixadas, código | base |
| `--ink` | `23% 0.02 260` | `#171d26` | Texto principal, botão primário | 16,2:1 sobre paper |
| `--ink-2` | `44% 0.02 260` | `#4c535e` | Texto secundário | 7,5:1 |
| `--ink-3` | `50% 0.02 260` | `#5d646f` | Placeholder, metadado | 5,8:1 |
| `--border` | `90% 0.008 260` | `#dbdee3` | Divisores (decorativo) | 1,3:1 (não é texto) |
| `--border-strong` | `62% 0.015 260` | `#81868f` | Borda de input e controle | 3,5:1 (passa 1.4.11) |
| `--signal` | `54% 0.19 38` | `#c43300` | Pede ação humana: badge do Inbox, aprovação pendente | 5,3:1 sobre paper |
| `--signal-bg` | `96% 0.03 50` | `#ffede0` | Fundo tingido do cartão de aprovação | `--signal` 4,8:1 |
| `--on-signal` | `99% 0.005 50` | `#fffbf9` | Texto sobre `--signal` | 5,4:1 |
| `--ok` | `48% 0.13 155` | `#00723b` | Concluído, conectado | 5,8:1 |
| `--err` | `50% 0.20 27` | `#bb0916` | Erro, recusado | 6,4:1 |

### 2.2 Escuro

| Token | OKLCH | Hex aprox. | Contraste |
|---|---|---|---|
| `--paper` | `17% 0.012 260` | `#0c1015` | base |
| `--surface` | `21% 0.014 260` | `#15181f` | base |
| `--sunken` | `14.5% 0.01 260` | `#080a0e` | base |
| `--ink` | `95% 0.008 260` | `#ebeff4` | 16,5:1 |
| `--ink-2` | `74% 0.015 260` | `#a5abb5` | 8,3:1 |
| `--ink-3` | `66% 0.015 260` | `#8d939c` | 6,2:1 |
| `--border` | `30% 0.015 260` | `#292e36` | decorativo |
| `--border-strong` | `55% 0.02 260` | `#6b727e` | 3,9:1 sobre paper, 3,7:1 sobre surface |
| `--signal` | `72% 0.17 45` | `#f97c3d` | 7,3:1 |
| `--signal-bg` | `26% 0.05 45` | `#381b0e` | `--signal` 6,0:1 |
| `--on-signal` | `17% 0.012 260` | `#0c1015` | 7,3:1 |
| `--ok` | `74% 0.15 155` | `#4bc680` | 8,8:1 |
| `--err` | `72% 0.17 25` | `#fd736d` | 7,2:1 |

### 2.3 Regras de cor

1. **Um destaque.** `--signal` significa "o humano precisa agir". Não é decoração, não colore agente, não vai em botão comum.
2. **Botão primário é tinta** (`--ink` com texto `--paper`). O destaque não é a cor dos botões.
3. **Agente não tem cor.** Cada agente tem **avatar-personagem** (ver §6.3). As 21 cores de papel (`--role-*`) e os 7 skins (incluindo "Cyberpunk") da v2.0 saem do caminho principal. Skins viram opção de Configurações, depois do lançamento, e nunca trocam `--signal`, `--ok` e `--err` (significado fixo).
4. Estado nunca só por cor: sempre ícone ou rótulo junto.
5. Sombras são tingidas do fundo (`oklch(20% 0.02 260 / .08)`), nunca preto puro.

### 2.4 Decisão de tema (impeccable "category reflex")

Dashboard de agentes "tem que ser azul escuro" é o reflexo de categoria. Evitamos o primeiro (escuro por padrão) e o segundo (azul-slate com destaque azul). O destaque quente e raro ajuda a separar "informação" de "ação".

## 3. Tipografia

| Papel | Fonte | Observação |
|---|---|---|
| Interface e leitura | **Geist** | Auto-hospedada (`@font-face`, `font-display: swap`). Nunca `<link>` do Google Fonts em produção (resolve o offline da v2.0). |
| Técnico | **Geist Mono** | Hashes, IDs, comandos, horários técnicos. |

Escala (razão 1,2, passo mínimo entre níveis 1,2):

| Token | Tamanho | Peso | Linha | Uso |
|---|---|---|---|---|
| `--text-xs` | 12px | 450 | 16px | Badge, horário |
| `--text-sm` | 14px | 450 | 20px | Metadado, lista |
| `--text-base` | 16px | 400 | 24px | Mensagem do chat, corpo |
| `--text-lg` | 19px | 560 | 26px | Título de Space, cartão |
| `--text-xl` | 23px | 600 | 30px | Título de página |

Regras: corpo com no máximo 72ch; `text-wrap: balance` em títulos e `text-wrap: pretty` em corpo; `font-variant-numeric: tabular-nums` em contadores, horários e métricas; `-webkit-font-smoothing: antialiased` na raiz; títulos sem caixa-alta; sem itálico em título.

## 4. Espaço, forma e profundidade

- **Espaço:** escala de 4px (`--space-1` a `--space-12`, mantida da v2.0). Ritmo variado: 8 dentro de grupos, 16 entre itens, 32 entre blocos.
- **Raio (uma regra só):** controles e botões **8px**; cartões e painéis **12px**; pílulas, avatares e badges **9999px**. Raio interno = raio externo menos o padding quando o padding for menor que o raio (ex.: cartão 12 com padding 4 contém elemento de 8).
- **Profundidade:** bordas para estrutura (divisores, input), sombra leve só para o que flutua (cabeçalho, popover, palette, toast). Cartões em repouso não têm sombra: separam-se por `--surface` sobre `--paper` e uma borda `--border`.
- **Sem cartão dentro de cartão.** Lista de Inbox e Activity são linhas com divisor, não cartões.
- **Imagens e avatares:** contorno de 1px `oklch(0 0 0 / .1)` no claro e `oklch(1 0 0 / .1)` no escuro (preto e branco puros, sem tinta).
- **Z-index:** escala documentada: conteúdo 0, rail 10, cabeçalho 20, popover 30, palette 40, toast 50.

## 5. Layout

### 5.1 Estrutura (desktop ≥ 1024px)

```
┌ Cabeçalho flutuante (pílula, 48px, centralizado) ─────────────────┐
│ Z  [ Chat ] Inbox 3  Activity            ⌘K   tema   avatar       │
└───────────────────────────────────────────────────────────────────┘
┌ Rail 248px ┐ ┌ Coluna principal (máx 760px, centrada) ┐ ┌ Computador 420px ┐
│ Spaces     │ │ Thread: mensagens, cartões, "Handled…" │ │ (abre sob demanda)│
│ Agentes    │ │                                        │ │                   │
│ Memória    │ │ Composer fixo no rodapé                │ │                   │
└────────────┘ └────────────────────────────────────────┘ └───────────────────┘
```

- **Cabeçalho:** flutuante, 48px de altura, 12px de distância do topo, largura do conteúdo (não da tela). Uma linha, nunca quebra.
- **Três abas:** `Chat`, `Inbox`, `Activity`. Esta é a navegação principal. Tudo o mais vive no Rail, no menu do avatar ou no palette.
- **Rail:** Spaces (lista), Agentes (lista com avatar e status), atalhos para Memória, Metas, DAG. Recolhe a 56px (só avatares).
- **Coluna principal:** 760px máximo, centrada. Nada de grade de cards como tela inicial.
- **Painel Computador:** abre à direita quando um agente usa o computador, e também aparece **inline** no chat como miniatura que expande (ver §6.6).
- **Altura:** `min-height: 100dvh`, nunca `100vh`.

### 5.2 Responsivo

| Largura | Comportamento |
|---|---|
| ≥ 1280 | Rail + Coluna + Computador lado a lado |
| 1024 a 1279 | Rail recolhido (56px); Computador sobrepõe a coluna como painel |
| 768 a 1023 | Sem Rail fixo (abre por botão); Computador vira folha inferior |
| < 768 | Uma coluna; **barra de abas inferior** com Chat, Inbox, Activity; Rail abre como folha; composer acima da barra; alvos ≥ 44px |

Cada layout multi-coluna declara o colapso no mesmo componente. Sem rolagem horizontal em 320, 375, 414 e 768.

## 6. Componentes

Todos com os 6 estados obrigatórios da v2.0 (carregando, vazio, erro, sucesso, validação, desabilitado), mantidos. A matriz completa continua em [DESIGN_SYSTEM.md §9](./DESIGN_SYSTEM.md); aqui só o que muda.

### 6.1 Cabeçalho flutuante e abas

- Aba ativa: fundo `--sunken`, texto `--ink`, peso 600. Inativa: `--ink-2`. Sem sublinhado azul.
- Contador do Inbox em pílula `--signal` com texto `--on-signal`, `tabular-nums`. Some quando é zero (não mostra "0").
- Troca de aba é instantânea (sem animação): ação frequente.

### 6.2 Mensagem (Chat)

- Mensagem do humano: alinhada à direita, fundo `--sunken`, raio 12. Mensagem do agente: sem balão, texto direto na coluna, com avatar de 28px e nome em `--text-sm`/600.
- Chamadas de ferramenta e "pensando" aparecem como linha recolhível em `--ink-3`, não como cartão.
- Código e hashes em Geist Mono sobre `--sunken`.

### 6.3 Avatar de agente

- Cada agente tem um **personagem** ilustrado (gerado com a ferramenta de imagem e revisado por humano), 28 / 40 / 64px, com contorno de 1px.
- Fallback obrigatório: monograma de uma letra em círculo `--sunken` (nunca emoji, nunca ícone de usuário genérico).
- Status vai em um indicador separado (ícone no canto), com rótulo para leitor de tela. O personagem não muda de cor por status.
- Os personagens ainda não existem: **pendência de produção**, ver §11.

### 6.4 Cartão de aprovação (substitui InlineApprovalCard v2.0)

```
┌─────────────────────────────────────────────────┐   fundo --signal-bg, borda 1px --signal
│ [ícone] Aprovação necessária        há 2 min    │
│ Backend Engineer quer publicar em produção      │   frase em linguagem humana
│ Risco ALTO · hash a3f8b2c1…                     │   mono para o hash
│ [ Recusar ]  [ Permitir uma vez ]  [ Sempre… ]  │
└─────────────────────────────────────────────────┘
```

- **Sem faixa lateral.** Destaque por fundo tingido, borda completa de 1px e ícone.
- **Três ações, sempre nesta ordem:** `Recusar` (secundário), `Permitir uma vez` (primário, tinta), `Sempre permitir` (secundário com ícone de escudo).
- `Sempre permitir` abre confirmação inline com o escopo exato ("permitir ações como esta para este agente neste Space") e fica **desabilitado com explicação** para risco ALTO ou quando a regra for ampla demais. **[DECISÃO do Victor, ver §10]**.
- Depois da decisão, o cartão colapsa em **recibo** de uma linha ("Permitido uma vez por você, 14:32") dentro do chat e entra em **Activity**. O recibo sobrevive a recarga (localStorage, 24h, mantido da v2.0).
- Hash divergente: aviso de validação em `--err` com ícone, botões de permitir desabilitados.
- Atalhos: `Enter` permite uma vez quando o cartão tem foco; `Esc` não recusa sozinho (recusar é explícito).

### 6.5 Inbox e Activity (linhas, não cartões)

- **Inbox:** tudo o que espera o humano (aprovações, senhas pedidas, perguntas de agentes), mais antigo no topo, agrupado por agente. Linha: avatar, frase, tempo, ação direta (os mesmos 3 botões nas aprovações).
- **Activity:** trilha somente leitura do que aconteceu, com a seção **"Resolvido por você e pelos agentes"** (equivalente ao "Handled for you"): o que os agentes fizeram sem precisar perguntar, com opção de desfazer quando existir. Filtros: agente, Space, tipo.
- Divisores `--border`, sem cartões. Linha de 56px, alvo ≥ 40px.

### 6.6 Painel Computador (substitui a tela Phone Relay)

- No chat, quando o agente usa o computador, aparece uma **miniatura** da sessão com o rótulo "Computador de <agente>". Clique expande no painel à direita ou na folha inferior.
- Estados: conectando (esqueleto com a forma do quadro), sem dispositivo (instrução de pareamento com QR), erro (screenshot de reserva + `Tentar de novo`), ao vivo (stream), controle liberado (camada "Controle liberado" com `Assumir controle`).
- Controles: `Assumir controle` / `Liberar`, qualidade (auto / baixa / alta), salvar quadro. Indicador "Você está no controle" com contagem do lease (5 min).
- **Sem moldura de navegador ou celular desenhada à mão.** O quadro é o conteúdo.

### 6.7 Command palette

- Atalho `⌘K` / `Ctrl+K`. **Sem animação de entrada nem saída** (ação repetida dezenas de vezes por dia); aparece e some instantaneamente.
- Centralizado, 640px, raio 16, sombra de popover, fundo `--surface` opaco, fundo atrás com escurecimento simples (sem blur em padrão).
- Grupos: Recentes, Páginas, Agentes, Spaces, Comandos. Setas, `Enter`, `Esc`.

### 6.8 Toast

- Canto inferior direito (em mobile, acima da barra de abas), 360px, até 3 visíveis. Sucesso e info somem em 5s; erro e aviso ficam até dispensar. Pausa no hover e com aba oculta.
- Variante por **ícone e texto**, não por faixa colorida.
- `aria-live="polite"` (erros: `assertive`).

### 6.9 Ícones

- **Phosphor**, peso "regular" (equivale a ~1,5px de traço), uma família só no app. `currentColor` sempre; estado pelo CSS (ativo = variante preenchida).
- Nenhum emoji como ícone ou status. Nenhum SVG de ícone desenhado à mão.

## 7. Vidro (material translúcido, ADR-010 revisado)

Permitido **só** no cabeçalho flutuante e, opcionalmente, atrás do palette. Rail, cartões, toasts e a coluna principal são **opacos**.

```css
.header-float {
  background: color-mix(in oklch, var(--paper) 78%, transparent);
  backdrop-filter: blur(16px) saturate(140%);
  border: 1px solid var(--border);
  box-shadow: 0 6px 24px oklch(20% 0.02 260 / .08);
}
@media (prefers-reduced-transparency: reduce) {
  .header-float { background: var(--paper); backdrop-filter: none; }
}
```

É uma aproximação web de vidro; não é o Liquid Glass da Apple (que só existe nas plataformas da Apple).

## 8. Movimento

Princípio: o app é de uso diário, então **menos movimento**. Cada animação precisa responder "o que isto comunica?".

| Interação | Frequência | Decisão |
|---|---|---|
| Troca de aba, palette abrir/fechar, atalhos de teclado | centenas por dia | **Sem animação** |
| Hover em linha ou botão | dezenas | Só cor, 120ms, atrás de `@media (hover: hover)` |
| Pressionar botão | muitas | `scale(0.96)`, 120ms `ease-out`, nunca abaixo de 0,95 |
| Cartão de aprovação entrando | ocasional | opacidade + `translateY(8px)`, 200ms `ease-out` |
| Cartão colapsando em recibo | ocasional | altura + opacidade, 200ms, saída mais rápida que entrada |
| Toast | ocasional | `translateX(16px)` + opacidade, 200ms; **transição CSS** (interrompível), não keyframe |
| Folha inferior, arrastar, painel Computador | gesto | **Mola** (duration 0,35s, bounce 0), mantém velocidade ao interromper |
| Esqueleto de carregamento | contínuo | brilho de 1,5s, ligado só enquanto carrega |

Curvas (nada de `ease-in`, nada de `transition: all`):

```css
--ease-out: cubic-bezier(0.23, 1, 0.32, 1);
--ease-in-out: cubic-bezier(0.77, 0, 0.175, 1);
```

Removidos da v2.0: elevação de 2px no hover do cartão, "ripple" no clique, entrada com mola no palette, sparkline animada no hover.

`prefers-reduced-motion: reduce`: remove movimento de posição e escala, mantém troca de opacidade e cor curtas (≤150ms). Animar só `transform` e `opacity`.

## 9. Acessibilidade e microdetalhes

- Contraste: tabelas de §2 (texto ≥ 4,5:1; borda de controle ≥ 3:1). Placeholder usa `--ink-3` (5,8:1 claro, 6,2:1 escuro): corrige o terciário `#64748b` da v2.0, que dava cerca de 3,8:1.
- Foco visível: anel 2px `--ink` com offset 2px, aparece sem animação. Nunca `outline: none` sem substituto.
- Alvos: ≥ 44px em toque, ≥ 40px em desktop denso (estender com pseudo-elemento se o visual for menor).
- Landmarks `nav`, `main`, `header`; botões só-ícone com `aria-label`; toast em região `aria-live`.
- Mensagens de erro humanas e acionáveis, com `Tentar de novo`; sem stack trace.
- `prefers-color-scheme` respeitado; tema aplicado antes da primeira pintura por arquivo externo (CSP sem script inline) para não piscar.
- Reduzir transparência e movimento respeitados (§7 e §8).

## 10. Decisões que dependem do Victor

| # | Decisão | Padrão proposto |
|---|---|---|
| D1 | `Sempre permitir` existe para risco ALTO? | **Não**: desabilitado com explicação; só `Permitir uma vez`. |
| D2 | Escopo da regra "sempre": por agente, por ação, por Space? | Por **agente + tipo de ação + Space**; revogável em Configurações. |
| D3 | Idioma da interface | PT-BR primeiro; rótulos curtos. |
| D4 | Skins extras (Ocean, Forest etc.) | Adiar para depois do lançamento; sem tocar `--signal`, `--ok`, `--err`. |
| D5 | Personagens dos agentes: gerar (imagem) ou usar monograma no lançamento | Monograma no lançamento, personagens na fase seguinte. |

## 11. Estado da implementação (2026-11-19)

**Feito e verificado no navegador (claro, escuro e celular 375px):**
- Tokens OKLCH com nomes legados mantidos, claro por padrão e escuro pelo sistema ou por `data-theme`; `ThemeContext` aplica `data-theme` (antes só a classe no `body`, que o CSS não lia, então o tema claro nunca aparecia); um script inline no `index.html` aplica o tema salvo antes da primeira pintura.
- Geist e Geist Mono auto-hospedadas; Phosphor em subconjunto de 53 glifos (6,6 KB) gerado por `scripts/build-icons.py`.
- Navegação: Chat, Inbox e Activity no cabeçalho e em barra inferior no celular; Inbox com contador ao vivo.
- Cartão de aprovação: Recusar, Permitir uma vez, Sempre permitir (confirmação inline; desabilitado para risco alto, decisão D1); regras permanentes com Revogar.
- Computador do agente como cartão no chat, com Expandir; sem frame em 3 s cai no screenshot estático (verificado: 1920x1080 carregado do gateway) e tenta reconectar a cada 5 s; o rótulo diz "Conectado, aguardando imagem" em vez de "Ao vivo" enquanto não há imagem.
- **Spaces**: Rail na sidebar para listar, criar, apagar e definir os agentes com acesso; o Space ativo (guardado no navegador) filtra Inbox, aprovações no chat, contador do cabeçalho e bots do chat. O "Geral" é aberto e protegido.
- **Resolvido por regras**: pedido coberto por uma regra "Sempre permitir" nasce aprovado, gera aviso e aparece na Activity com "Revogar regra".
- Emoji do chrome (toast, sidebar, chat, VNC, palette, cards, erro) trocados por Phosphor (62 glifos, 7,7 KB).
- Palette sem animação; hover do card de agente sem elevação; 12 faixas laterais coloridas viraram borda completa.

**Divergências em relação à especificação:**
- Spaces filtram por **nome de agente** (bot_id). Conversas (threads e chat) e notas de memória também são por Space; o Geral mostra tudo. O painel de Threads e o de Memória (módulo extras, sem o contexto de Spaces) leem o Space de `localStorage` e remontam quando ele muda (`SpaceScopedHost`).
- "Resolvido por regras" substitui "Resolvido por você e pelos agentes": só o que uma regra liberou é registrado, e o controle é **Revogar regra**. `Desfazer` não existe porque a ação já foi liberada.
- Activity mostra decisões da pessoa (recibos de 7 dias) e os avisos do SSE **desta sessão** (em memória).
- Avatares-personagem (D5): ficou o monograma/ícone; os personagens não foram produzidos.
- Cerca de 110 emoji de **conteúdo** continuam (reações, tipos de arquivo, status de painéis secundários); só o chrome foi migrado.
- A cor de papel por função (`--role-*`) e os skins continuam no código como dados; deixaram de colorir a interface principal.
- Contraste foi calculado para os tokens; o Lighthouse mede Accessibility 100.

**Medições (release, Lighthouse mobile, servido com brotli):** Performance 91 (FCP 0,9 s, LCP 3,5 s, TBT 40 ms, CLS 0), Accessibility 100, Best Practices 96, SEO 63 (robots.txt bloqueia indexação de propósito). Dois ajustes: trocar o seletor `body:has()` do splash por remoção via código (TBT 2.180 para 60 ms) e dividir o wasm em dois módulos, principal e extras sob demanda (ADR-011): a abertura baixa 958 KB bruto (309 KB brotli) em vez de 2,22 MB (677 KB).

**Decisões D1 a D5:** adotados os padrões propostos em §10 (D1 sem "Sempre permitir" para risco alto; D2 escopo agente + ferramenta + categoria no backend; D3 PT-BR; D4 skins adiados; D5 monograma). Revise se algum não for o que você quer.

## 12. O que mudou em relação à v2.0

| v2.0 | v2.1 |
|---|---|
| Só escuro, slate + azul, Inter | Claro e escuro, neutros frios tingidos, destaque "sinal", Geist |
| 21 cores de papel por agente | Avatar-personagem por agente, cor só para significado |
| 11 telas, sidebar de 4 grupos | 3 abas + Rail; telas viram Spaces ou vistas |
| Vidro em header, sidebar, palette, toast | Vidro só no cabeçalho flutuante |
| Cartão de aprovação com faixa lateral 4px, 3 botões Aprovar/Rejeitar/Escalar | Fundo tingido + borda completa; Recusar / Permitir uma vez / Sempre permitir |
| Phone Relay como página | Painel Computador inline no chat |
| Palette com mola de entrada; cartão sobe no hover; ripple | Palette sem animação; sem elevação e sem ripple |
| Emoji como ícone, ASCII quebrado | Phosphor, wireframes e mockup real |
| Contraste informado sem cálculo (vermelho 3,9:1 declarado como 4,5:1) | Contrastes calculados e tabelados |

## 13. Changelog

| Versão | Data | Mudança |
|---|---|---|
| 2.1.0 | 2026-11-19 | Proposta de nova direção visual baseada na auditoria da v2.0 e nos repositórios de referência |
