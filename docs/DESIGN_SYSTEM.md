# Zylos Cockpit Design System

**Version:** 2.0.0  
**Last Updated:** 2026-11-11 (Fase 9 — Fases 1–4 completas)  
**Inspired by:** OpenBot, OpenDots, Paperclip, Rerun, Ruflo, ECC, Agency Agents, OpenDuet, OpenMuse, OpenMausBot

---

## Design Philosophy

The Zylos Cockpit is a **mission control center** for AI agent teams. It follows three core principles:

1. **Agent-First Navigation**: Every interaction starts with selecting an agent or channel
2. **Contextual Awareness**: Real-time status, pending approvals, and task progress always visible
3. **Progressive Disclosure**: Simple by default, powerful when needed

---

## Color Palette

### Core Colors (Dark Theme Default)

| Token | Value | Usage |
|-------|-------|-------|
| `--bg-base` | `#0a0a0f` | Primary background |
| `--bg-surface` | `#1a1a2e` | Cards, panels, sidebar |
| `--bg-elevated` | `#252540` | Hover states, active items |
| `--bg-accent` | `#6366f1` | Primary actions, selections |
| `--text-primary` | `#e2e8f0` | Main text |
| `--text-secondary` | `#94a3b8` | Muted text, labels |
| `--text-disabled` | `#64748b` | Disabled states |
| `--border-default` | `#334155` | Borders, dividers |
| `--border-accent` | `#6366f1` | Active borders |

### Status Colors

| Token | Value | Usage |
|-------|-------|-------|
| `--status-success` | `#10b981` | Completed tasks, online agents |
| `--status-warning` | `#f59e0b` | Pending approvals, warnings |
| `--status-error` | `#ef4444` | Errors, failed tasks |
| `--status-info` | `#3b82f6` | Information, progress |
| `--status-idle` | `#6b7280` | Idle agents, inactive |

### Agent Identity Colors (8 divisions)

| Division | Color |
|----------|-------|
| Engineering | `#6366f1` (Indigo) |
| Research | `#8b5cf6` (Violet) |
| Operations | `#ec4899` (Pink) |
| Creative | `#f97316` (Orange) |
| Security | `#ef4444` (Red) |
| Management | `#14b8a6` (Teal) |
| Support | `#3b82f6` (Blue) |
| Custom | `#a855f7` (Purple) |

---

## Typography

### Font Stack

```css
--font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
--font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
```

### Type Scale

| Token | Size | Line Height | Weight | Usage |
|-------|------|-------------|--------|-------|
| `--text-xs` | 11px | 16px | 400 | Labels, timestamps |
| `--text-sm` | 13px | 18px | 400 | Secondary text, metadata |
| `--text-base` | 14px | 20px | 400 | Body text |
| `--text-lg` | 16px | 24px | 500 | Section headers |
| `--text-xl` | 18px | 28px | 600 | Page headers |
| `--text-2xl` | 24px | 32px | 700 | Hero headers |

---

## Spacing System

Based on 4px grid, using rem units.

| Token | Value | Usage |
|-------|-------|-------|
| `--space-1` | 4px | Tight spacing |
| `--space-2` | 8px | Icon padding |
| `--space-3` | 12px | Component padding |
| `--space-4` | 16px | Card padding |
| `--space-5` | 20px | Section spacing |
| `--space-6` | 24px | Panel gaps |
| `--space-8` | 32px | Layout gaps |
| `--space-10` | 40px | Page margins |

---

## Layout Structure

### 3-Column Chat-Centric Layout

```
┌─────────────┬──────────────────────────┬─────────────┐
│  Sidebar    │      Chat Area           │  Context    │
│  (240px)    │      (flex-1)            │  (320px)    │
│             │                          │             │
│  • Agents   │  • Message bubbles       │  • Agent    │
│  • Channels │  • Approval cards        │    profile  │
│  • Spaces   │  • Tool outputs          │  • Task     │
│  • Routines │  • Input area            │    details  │
│             │                          │  • Files    │
│             │                          │  • Memory   │
└─────────────┴──────────────────────────┴─────────────┘
```

### Breakpoints

| Breakpoint | Width | Layout |
|------------|-------|--------|
| Mobile | < 768px | Single column, sidebar as drawer |
| Tablet | 768-1024px | 2 columns (sidebar + chat) |
| Desktop | ≥ 1024px | 3 columns |

---

## Component Specifications

### Message Bubbles

**User Message:**
- Background: `--bg-accent` (indigo)
- Text: white
- Alignment: right
- Border radius: 12px (top-left, top-right, bottom-left)
- Max width: 70%

**Agent Message:**
- Background: `--bg-surface`
- Text: `--text-primary`
- Alignment: left
- Border radius: 12px (top-right, bottom-right, bottom-left)
- Avatar: 32px circle, left-aligned
- Max width: 75%

**System Message:**
- Background: transparent
- Text: `--text-secondary`
- Alignment: center
- Font size: `--text-sm`
- Border: 1px dashed `--border-default`

### Approval Gate Card

Inline card for HITL approvals with 3 action buttons.

```
┌─────────────────────────────────────────────┐
│ ⚠️  Approval Required                        │
│                                             │
│ The agent wants to execute:                 │
│ • Send email to client@example.com          │
│ • Subject: "Project Update Q4"              │
│                                             │
│ [Allow Once] [Always Allow] [Reject]        │
└─────────────────────────────────────────────┘
```

- Background: `--bg-elevated`
- Border: 2px solid `--status-warning`
- Border radius: 8px
- Padding: 16px
- Action buttons: small, right-aligned

### Agent Status Indicator

Small dot (8px) with color based on status:
- 🟢 Success: agent is running/active
- 🟡 Warning: pending approval needed
- 🔴 Error: agent failed
- ⚪ Idle: agent is inactive

### Fleet Auditor Card

Bot cost/efficacy metrics card with anomaly alerts.

```
┌─────────────────────────────────────┐
│ 🤖 bot-analyst                 ⚠️  │
│                                   │
│ Cost: $12.40    Actions: 1,847    │
│ Success: 94.2%  Avg: 6.7s         │
│                                   │
│ ⚠ Cost above threshold ($10)      │
└─────────────────────────────────────┘
```

- Background: `--bg-surface`
- Border: 1px solid `--border-default`; alert state: `--status-warning`
- Border radius: `--radius-md`
- Metrics grid: 2 columns, `--text-sm` mono values
- Alert badge: yellow background, top-right

### Federation Node Card

Federated node status with HLC and VectorClock display.

```
┌─────────────────────────────────────┐
│ ✅ US-East Primary    us-east-1     │
│ Sincronizado                        │
│ HLC: 1731340800000.000042           │
│ VC: [100,98,95,87]                  │
│ Latência: 12ms                      │
└─────────────────────────────────────┘
```

- Left border 3px colored by status (green=synced, cyan=syncing, yellow=drifted, red=offline/conflict)
- HLC/VC values in `--font-mono`, `--text-sm`
- Pending conflicts badge: red pill, right-aligned

### Voice Panel Waveform

Audio visualization with phase-based coloring.

- Container: 64px height, `--bg-surface`, `--radius-md`
- Bars: 4px wide, animated height 8-48px
- Phase colors: listening=`--status-info`, speaking=`--status-success`, processing=`--status-warning`, error=`--status-error`
- Animation: `zvoicepulse` alternate infinite

### Agent Avatar Grid

Division-colored avatars with status indicators.

- Sizes: 48px (default), 32px (compact), 64px (large)
- Division colors: Engineering=#6366f1, Research=#8b5cf6, Operations=#ec4899, Creative=#f97316, Security=#ef4444, Management=#14b8a6, Support=#3b82f6, Custom=#a855f7
- Status dot: 12px, bottom-right, 2px border `--bg-base`
- Status colors: online=`--status-success`, busy=`--status-error`, away=`--status-warning`, offline=`--status-idle`

### Skin Theme Card

Theme preview card with active state indicator.

```
┌──────────────────────────┐
│ ████████████████████████ │ ← preview bar (32px)
│                          │
│ Cyberpunk Neon      ✓   │
│ Dark theme with neon... │
└──────────────────────────┘
```

- Active state: `--border-accent` + `--shadow-glow-blue`
- Preview bar: 32px height, theme-specific gradient
- Active badge: absolute top-right, `--bg-accent` background

### Sidebar Item

```
┌──────────────────────────┐
│ [Avatar] Agent Name      │
│          Status • Tasks   │
└──────────────────────────┘
```

- Height: 48px
- Padding: 12px 16px
- Hover: `--bg-elevated`
- Active: `--bg-accent` with left border 3px
- Avatar: 32px circle
- Unread badge: red circle, top-right of avatar

---

## Shadows & Elevation

| Level | Shadow | Usage |
|-------|--------|-------|
| `--shadow-sm` | `0 1px 2px rgba(0,0,0,0.3)` | Buttons, inputs |
| `--shadow-md` | `0 4px 6px rgba(0,0,0,0.4)` | Cards, panels |
| `--shadow-lg` | `0 10px 15px rgba(0,0,0,0.5)` | Modals, dropdowns |

---

## Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 4px | Buttons, inputs |
| `--radius-md` | 8px | Cards, panels |
| `--radius-lg` | 12px | Message bubbles |
| `--radius-full` | 9999px | Avatars, pills |

---

## Motion & Transitions

| Token | Value | Usage |
|-------|-------|-------|
| `--transition-fast` | 100ms ease-out | Hover states |
| `--transition-base` | 200ms ease-out | Most transitions |
| `--transition-slow` | 300ms ease-out | Panel slides |

### Spring Animation (for message appearance)

```css
@keyframes message-in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
```

Duration: 200ms, easing: `cubic-bezier(0.4, 0, 0.2, 1)`

---

## Icons

Use Lucide icons (consistent with OpenBot/OpenDots):
- Size: 16px (inline), 20px (buttons), 24px (headers)
- Stroke width: 2px
- Color: inherit from text

---

## Accessibility

- All interactive elements have `aria-label`
- Color contrast ratio ≥ 4.5:1 for text
- Focus rings: 2px solid `--bg-accent`, offset 2px
- Keyboard navigation: Tab, Enter, Escape, Arrow keys
- Reduced motion: respect `prefers-reduced-motion`

---

## Dark/Light Theme

Dark theme is default. Light theme inverts:
- `--bg-base`: `#ffffff`
- `--bg-surface`: `#f8fafc`
- `--bg-elevated`: `#f1f5f9`
- `--text-primary`: `#0f172a`
- `--text-secondary`: `#475569`

Toggle via `data-theme="light"` on `<html>`.

---

## Cross-References

- **APP_FLOW.md**: State machine for all 17 tabs
- **DESIGN_SYSTEM.md**: This file (tokens and components)
- **PRD.md**: Product requirements
- **ARCHITECTURE.md**: Technical architecture