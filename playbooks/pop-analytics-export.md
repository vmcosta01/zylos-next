# POP-ANALYTICS-002: Exportar Analytics (CSV e PDF)

**Version:** 1.0.0
**Playbook:** [pb-analytics-preset-reports.md](./pb-analytics-preset-reports.md)
**Last Updated:** 2026-11-17
**Fase:** 11.10 (Analytics Dashboard)
**Crate:** `crates/zylos-analytics`

## Purpose

Exportar dados do Analytics Dashboard para análise externa (planilha, BI) em **CSV**, e relatórios para compartilhamento em **PDF A4**. Implementação: `crates/zylos-analytics/src/export.rs` e `crates/zylos-analytics/src/pdf.rs`.

## Trigger

- Pedido de dados para análise fora do Zylos
- Revisão semanal/mensal com stakeholders (PDF)
- Evidência para auditoria de desempenho de agentes

## Steps

### 1. CSV de dados brutos / agregados

```bash
SINCE=$(( $(date +%s) - 7*86400 ))
# eventos brutos (todas as colunas + dimensões)
curl -s "http://localhost:3000/api/v1/analytics/export?kind=events&format=csv&since=$SINCE" -o eventos_7d.csv
# série temporal na granularidade escolhida
curl -s "http://localhost:3000/api/v1/analytics/export?kind=timeseries&granularity=daily&since=$SINCE" -o serie.csv
# agregado por skill (ou by=agent)
curl -s "http://localhost:3000/api/v1/analytics/export?kind=breakdown&by=skill&since=$SINCE" -o por_skill.csv
```

Formato do CSV:
- Linhas iniciais `# chave: valor` com **metadados** (tipo, filtro aplicado em texto, momento de geração, total de linhas). Ferramentas como `pandas.read_csv(..., comment='#')` ignoram essas linhas.
- Cabeçalho + linhas no padrão RFC 4180 (aspas duplicadas, campos com `,`/`"`/quebra entre aspas).
- **Proteção contra CSV/formula injection**: células de texto que começam com `=`, `+`, `-` ou `@` recebem prefixo `'` (números como `-3.5` não são alterados).
- Timestamps em ISO-8601 UTC (`2026-11-17T00:00:00Z`).

No cockpit, os botões **CSV série / CSV por skill / CSV eventos** baixam exatamente o recorte dos filtros visíveis.

### 2. PDF de relatório

```bash
curl -s "http://localhost:3000/api/v1/analytics/reports/weekly_bot_performance?format=pdf" -o weekly.pdf
curl -s "http://localhost:3000/api/v1/analytics/reports/monthly_cost_analysis?format=pdf&agents=atlas" -o custo_atlas.pdf
```

O PDF é A4 (595×842 pt), Helvetica, paginado, com cabeçalho (título, período), tabelas, barras e rodapé `Página N`.

**Por que um escritor PDF próprio** (`pdf.rs`, ~300 linhas) em vez de `printpdf`/`genpdf`: o relatório só precisa de texto, linhas e retângulos; um writer PDF 1.4 mínimo evita dependências transitivas (fontes, `lopdf`, imagem), mantém build/compile rápido e gera saída **determinística** (mesma entrada → mesmos bytes), o que facilita testes. Limitação: fonte padrão Helvetica com WinAnsi — caracteres fora de Latin-1 viram `?`.

### 3. PDF de dados brutos

Não suportado de propósito (`/analytics/export?format=pdf` → HTTP 400): volume de eventos brutos não cabe em PDF útil. Use um relatório preset.

## Verification

- [ ] CSV abre em planilha com acentos corretos (UTF-8)
- [ ] Linhas `#` mostram o filtro aplicado
- [ ] PDF abre e tem tamanho A4; `file weekly.pdf` → `PDF document, version 1.4`
- [ ] Export de 7d com ~240k eventos: medido **7.1 s / 27.9 MB** no benchmark — para volumes maiores, estreite o filtro
