# Design System — Zylos-Next Cockpit

## Tabela de Tokens

### Cores (Dark Theme — Engineering Dashboard)
| Token | Valor Hex | Valor HSL | Uso |
|-------|-----------|-----------|-----|
| `--bg-primary` | `#0A0E1A` | `hsl(225, 45%, 7%)` | Background principal |
| `--bg-secondary` | `#111827` | `hsl(220, 38%, 11%)` | Cards/Panels |
| `--bg-tertiary` | `#1F2937` | `hsl(217, 33%, 17%)` | Hover states |
| `--fg-primary` | `#F9FAFB` | `hsl(210, 40%, 98%)` | Texto principal |
| `--fg-secondary` | `#9CA3AF` | `hsl(218, 11%, 65%)` | Texto secundário |
| `--fg-muted` | `#6B7280` | `hsl(220, 9%, 46%)` | Texto desabilitado |
| `--accent-blue` | `#3B82F6` | `hsl(217, 91%, 60%)` | Ações primárias |
| `--accent-cyan` | `#06B6D4` | `hsl(188, 95%, 43%)` | Links/Info |
| `--accent-green` | `#10B981` | `hsl(160, 84%, 39%)` | Success/Verified |
| `--accent-yellow` | `#F59E0B` | `hsl(38, 92%, 50%)` | Warning/Pending |
| `--accent-red` | `#EF4444` | `hsl(0, 84%, 60%)` | Error/Refuted |
| `--accent-purple` | `#8B5CF6` | `hsl(258, 90%, 66%)` | Agent/Special |
| `--border` | `#374151` | `hsl(215, 16%, 27%)` | Bordas |
| `--border-focus` | `#60A5FA` | `hsl(213, 94%, 68%)` | Foco ativo |

### Tipografia
| Token | Família | Tamanho | Peso | Uso |
|-------|---------|---------|------|-----|
| `--font-sans` | `Inter, system-ui` | - | - | Base |
| `--font-mono` | `JetBrains Mono, Fira Code` | - | - | Código |
| `--text-xs` | - | `12px` | `400` | Labels |
| `--text-sm` | - | `14px` | `400` | Body |
| `--text-base` | - | `16px` | `400` | Paragraph |
| `--text-lg` | - | `18px` | `500` | Subheading |
| `--text-xl` | - | `20px` | `600` | Heading 3 |
| `--text-2xl` | - | `24px` | `600` | Heading 2 |
| `--text-3xl` | - | `30px` | `700` | Heading 1 |
| `--text-code` | - | `13px` | `400` | Inline code |

### Espaçamento (Escala Modular 4px)
| Token | Valor | Uso |
|-------|-------|-----|
| `--space-1` | `4px` | Micro gaps |
| `--space-2` | `8px` | Padding interno |
| `--space-3` | `12px` | Padding de botões |
| `--space-4` | `16px` | Padding de cards |
| `--space-6` | `24px` | Margem entre seções |
| `--space-8` | `32px` | Margem entre painéis |
| `--space-12` | `48px` | Margem de página |

### Sombras (Elevações)
| Token | Valor | Uso |
|-------|-------|-----|
| `--shadow-sm` | `0 1px 2px rgba(0,0,0,0.3)` | Botões |
| `--shadow-md` | `0 4px 6px rgba(0,0,0,0.4)` | Cards |
| `--shadow-lg` | `0 10px 15px rgba(0,0,0,0.5)` | Modais |
| `--shadow-glow-blue` | `0 0 20px rgba(59,130,246,0.3)` | Active elements |

### Border-radius
| Token | Valor | Uso |
|-------|-------|-----|
| `--radius-sm` | `4px` | Botões pequenos |
| `--radius-md` | `6px` | Botões, inputs |
| `--radius-lg` | `8px` | Cards |
| `--radius-xl` | `12px` | Modais |
| `--radius-full` | `9999px` | Pills, avatares |

### Breakpoints Responsivos
| Token | Valor | Uso |
|-------|-------|-----|
| `--bp-sm` | `640px` | Mobile landscape |
| `--bp-md` | `768px` | Tablet |
| `--bp-lg` | `1024px` | Laptop |
| `--bp-xl` | `1280px` | Desktop |
| `--bp-2xl` | `1536px` | Large desktop |

## Estados de UI Obrigatórios por Componente

### 1. Button
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Default** | `--accent-blue` bg, `--fg-primary` text | Cursor pointer |
| **Hover** | `brightness(1.1)`, `--shadow-glow-blue` | Transição 150ms |
| **Active** | `brightness(0.95)`, `scale(0.98)` | Feedback tátil |
| **Loading** | Spinner SVG, text disabled | Pointer events none |
| **Disabled** | `--bg-tertiary` bg, `--fg-muted` text | Cursor not-allowed |
| **Error** | `--accent-red` bg, shake animation | Tooltip com erro |

### 2. Card (Agente/Workflow)
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Empty** | Border dashed, ícone + "No agents" | CTA para criar |
| **Loading** | Skeleton pulse animation | 3 cards placeholder |
| **Success** | Border `--accent-green`, badge verified | Click expande |
| **Error** | Border `--accent-red`, ícone warning | Retry button |
| **Validation** | Border `--accent-yellow`, inline error | Focus no campo |
| **Disabled** | Opacity 0.5, grayscale filter | Tooltip razão |

### 3. Input (Text/Select)
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Default** | Border `--border`, `--bg-secondary` bg | Focus ring off |
| **Focus** | Border `--border-focus`, `--shadow-glow-blue` | Outline none |
| **Empty** | Placeholder `--fg-muted` | - |
| **Error** | Border `--accent-red`, mensagem abaixo | Shake on submit |
| **Success** | Border `--accent-green`, checkmark | - |
| **Disabled** | `--bg-tertiary` bg, cursor not-allowed | - |

### 4. Agent Status Badge
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Idle** | `--fg-muted` dot, "Idle" text | - |
| **Running** | `--accent-cyan` dot pulse, "Running" | Tooltip: tempo |
| **Success** | `--accent-green` dot, "Verified" | Click: ver proof |
| **Error** | `--accent-red` dot, "Refuted" | Click: ver reason |
| **Pending** | `--accent-yellow` dot spin, "Pending" | Tooltip: aguardando |
| **Disabled** | Gray dot, "Parked" | - |

### 5. DAG Node (Workflow)
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Pending** | Gray border, opacity 0.6 | - |
| **Running** | Blue pulse border, spinner | - |
| **Success** | Green border, checkmark | Click: ver output |
| **Error** | Red border, X icon | Click: ver error |
| **Skipped** | Dashed border, opacity 0.4 | Tooltip: razão |
| **Rollback** | Orange border, undo icon | Auto-highlight |

### 6. Toast/Notification
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Info** | `--accent-cyan` left border | Auto-dismiss 5s |
| **Success** | `--accent-green` left border | Auto-dismiss 3s |
| **Warning** | `--accent-yellow` left border | Persist até click |
| **Error** | `--accent-red` left border | Persist + Retry btn |
| **Loading** | Spinner, no auto-dismiss | Manual dismiss |
| **Action** | Primary button inside | Click executa |

## Componentes Base

### Button
```rust
// Leptos component
<Button variant="primary" state="loading" on_click=...>
    "Save Agent"
</Button>
```

### Card
```rust
<Card state="success" badge="Verified">
    <CardHeader title="Memory Agent" />
    <CardBody>...</CardBody>
</Card>
```

### Badge
```rust
<Badge variant="success" pulse=true>
    "Running"
</Badge>
```

## Acessibilidade (Obrigatória)
- **Keyboard navigation:** Tab order lógico, Enter/Space ativa
- **Screen reader:** aria-label em todos elementos interativos
- **Color contrast:** WCAG AA (4.5:1 texto, 3:1 UI)
- **Focus visible:** `--border-focus` sempre visível
- **Reduced motion:** Respeita `prefers-reduced-motion`

## Referências
- [APP_FLOW.md](./APP_FLOW.md) para jornada do usuário
- [PRD.md](./PRD.md) para features
- Leptos 0.7 docs para implementação
