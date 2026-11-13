# POP: Skill Save & Validation (Fase 10.4)

## Objetivo
Procedimento operacional para salvar skills via Skill Editor Panel com validação YAML em tempo real e integração ao backend Hermes.

## Escopo
- Validação local de YAML antes do envio
- Syntax highlighting no preview (parser WASM simples)
- Undo/redo com stack de estados (máx. 20 entradas)
- POST para `/api/v1/hermes/skill-evolution/contracts`

## Pré-condições
- Cockpit rodando (`trunk serve` ou build WASM)
- Gateway Hermes acessível em `localhost:3000`
- Skill selecionada no painel esquerdo

## Procedimento

### 1. Editar Skill
1. Selecione uma skill na lista lateral
2. Adicione steps via botão **+ Add Step**
3. Observe o preview YAML com syntax highlighting à direita
4. Use **↩ Undo** / **↪ Redo** para navegar pelo histórico de edições

### 2. Validar Localmente
- O parser YAML roda automaticamente a cada mudança
- Erros aparecem inline abaixo da área de steps:
  - `YAML vazio` — preview sem conteúdo
  - `Campo 'name' obrigatório` — chave `name` ausente
  - `L{n}: tabulação não permitida` — tabs detectadas
  - `Chave duplicada no nível raiz` — heurística de duplicata
- **Não prossiga se houver erros de validação**

### 3. Salvar no Backend
1. Clique em **💾 Save to Backend**
2. O sistema valida localmente antes de enviar
3. Se válido, faz POST com payload `{ name, description, category, yaml }`
4. Feedback visual por 3 segundos:
   - ✅ `Salvo com sucesso` — HTTP 2xx
   - ❌ `Erro HTTP {code}` — resposta não-OK
   - ❌ `Falha de conexão` — gateway indisponível
   - ❌ `Validação falhou: {erro}` — bloqueado localmente

## Classes CSS de Syntax Highlighting
| Classe | Elemento |
|---|---|
| `yaml-key` | Chaves de mapeamento |
| `yaml-string` | Strings literais |
| `yaml-number` | Números |
| `yaml-bool` | true/false/yes/no/null |
| `yaml-comment` | Comentários `#` |
| `yaml-dash` | Marcador de lista `-` |
| `yaml-colon` | Separador `:` |

## Troubleshooting
| Sintoma | Causa Provável | Ação |
|---|---|---|
| "Gateway indisponível" | Backend offline | Verificar `localhost:3000` |
| "Nenhuma skill encontrada" | API retornou lista vazia | Verificar endpoint `/contracts` |
| Highlighting quebrado | YAML malformado | Corrigir erros de validação primeiro |
| Undo não funciona | Stack vazia | Fazer pelo menos uma edição |

## Referências
- [IMPLEMENTATION_PLAN_FASE10.md](../../docs/IMPLEMENTATION_PLAN_FASE10.md) §10.4
- Componente: `apps/zylos-cockpit/src/components/skill_editor_panel.rs`