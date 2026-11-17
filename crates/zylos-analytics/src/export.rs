//! Export CSV/PDF (T11.10.4).
//!
//! - CSV segue RFC 4180 (`,` separador, `"` com escape por duplicação, CRLF).
//!   Linhas iniciadas por `#` no topo carregam metadata (título, filtro,
//!   gerado em, total de linhas). Células iniciadas por `= + - @` recebem
//!   prefixo `'` para neutralizar CSV/formula injection em planilhas.
//! - PDF A4 retrato via [`crate::pdf`] — cabeçalho, metadata, seções com
//!   tabelas (paginação automática e cabeçalho repetido) e gráficos de barras.

use crate::aggregations::{Bucket, GroupStats};
use crate::event::AnalyticsEvent;
use crate::filter::Filter;
use crate::pdf::{PdfDoc, A4_H, A4_W};
use crate::reports::{Report, Section};
use chrono::{DateTime, Utc};

/// Formata timestamp Unix como ISO-8601 UTC.
pub fn iso(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| ts.to_string())
}

/// Escapa uma célula CSV.
pub fn csv_cell(s: &str) -> String {
    let guarded = if s.starts_with(['=', '+', '-', '@']) && s.parse::<f64>().is_err() {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

fn csv_row(cells: &[String]) -> String {
    let mut r = cells
        .iter()
        .map(|c| csv_cell(c))
        .collect::<Vec<_>>()
        .join(",");
    r.push_str("\r\n");
    r
}

fn meta_lines(title: &str, filter: &Filter, generated_at: i64, rows: usize) -> String {
    format!(
        "# {}\r\n# filtro: {}\r\n# gerado_em: {}\r\n# linhas: {rows}\r\n",
        title.replace(['\r', '\n'], " "),
        filter.describe().replace(['\r', '\n'], " "),
        iso(generated_at)
    )
}

/// Exporta eventos brutos como CSV (todas as colunas + dimensões como JSON).
pub fn events_to_csv(events: &[AnalyticsEvent], filter: &Filter, generated_at: i64) -> String {
    let mut out = meta_lines("Eventos de analytics", filter, generated_at, events.len());
    out.push_str(&csv_row(
        &[
            "id",
            "timestamp",
            "agent",
            "skill",
            "success",
            "latency_ms",
            "cost_usd",
            "tokens",
            "dimensions",
        ]
        .map(String::from),
    ));
    for e in events {
        out.push_str(&csv_row(&[
            e.id.map(|i| i.to_string()).unwrap_or_default(),
            iso(e.ts),
            e.agent.clone(),
            e.skill.clone(),
            e.success.to_string(),
            e.latency_ms.to_string(),
            format!("{:.6}", e.cost_usd),
            e.tokens.to_string(),
            serde_json::to_string(&e.dimensions).unwrap_or_default(),
        ]));
    }
    out
}

fn bucket_cells(b: &Bucket) -> Vec<String> {
    vec![
        b.count.to_string(),
        b.success.to_string(),
        format!("{:.4}", b.success_rate),
        format!("{:.2}", b.avg_latency_ms),
        b.max_latency_ms.to_string(),
        format!("{:.6}", b.cost_usd),
        b.tokens.to_string(),
    ]
}

const METRIC_HEADERS: [&str; 7] = [
    "count",
    "success",
    "success_rate",
    "avg_latency_ms",
    "max_latency_ms",
    "cost_usd",
    "tokens",
];

/// Exporta série temporal como CSV.
pub fn buckets_to_csv(
    title: &str,
    buckets: &[Bucket],
    filter: &Filter,
    generated_at: i64,
) -> String {
    let mut out = meta_lines(title, filter, generated_at, buckets.len());
    let mut h = vec!["bucket_start".to_string()];
    h.extend(METRIC_HEADERS.map(String::from));
    out.push_str(&csv_row(&h));
    for b in buckets {
        let mut r = vec![iso(b.start)];
        r.extend(bucket_cells(b));
        out.push_str(&csv_row(&r));
    }
    out
}

/// Exporta breakdown (por agente/skill) como CSV.
pub fn groups_to_csv(
    title: &str,
    key: &str,
    groups: &[GroupStats],
    filter: &Filter,
    generated_at: i64,
) -> String {
    let mut out = meta_lines(title, filter, generated_at, groups.len());
    let mut h = vec![key.to_string()];
    h.extend(METRIC_HEADERS.map(String::from));
    out.push_str(&csv_row(&h));
    for g in groups {
        let mut r = vec![g.key.clone()];
        r.extend(bucket_cells(&g.stats));
        out.push_str(&csv_row(&r));
    }
    out
}

/// Exporta um [`Report`] como CSV (uma tabela por seção, separadas por linha vazia).
pub fn report_to_csv(r: &Report) -> String {
    let mut out = format!(
        "# {}\r\n# {}\r\n# periodo: {} -> {}\r\n# gerado_em: {}\r\n",
        r.title,
        r.subtitle,
        iso(r.period_start),
        iso(r.period_end),
        iso(r.generated_at)
    );
    for s in &r.sections {
        match s {
            Section::Table {
                heading,
                columns,
                rows,
            } => {
                out.push_str(&format!("\r\n# {heading}\r\n"));
                out.push_str(&csv_row(columns));
                for row in rows {
                    out.push_str(&csv_row(row));
                }
            }
            Section::Bars {
                heading,
                unit,
                items,
            } => {
                out.push_str(&format!("\r\n# {heading}\r\n"));
                out.push_str(&csv_row(&["label".into(), format!("value_{unit}")]));
                for (l, v) in items {
                    out.push_str(&csv_row(&[l.clone(), format!("{v:.4}")]));
                }
            }
            Section::Paragraph { heading, text } => {
                out.push_str(&format!("\r\n# {heading}\r\n"));
                out.push_str(&csv_row(&["texto".into()]));
                out.push_str(&csv_row(std::slice::from_ref(text)));
            }
        }
    }
    out
}

const MARGIN: f64 = 50.0;
const BOTTOM: f64 = 60.0;
const ACCENT: (f64, f64, f64) = (0.16, 0.38, 0.74);
const GREY: (f64, f64, f64) = (0.6, 0.6, 0.6);
const ZEBRA: (f64, f64, f64) = (0.95, 0.96, 0.98);

struct Cursor<'a> {
    doc: PdfDoc,
    y: f64,
    page: usize,
    report: &'a Report,
}

impl<'a> Cursor<'a> {
    fn header(&mut self) {
        let r = self.report;
        self.doc.rect_fill(0.0, A4_H - 8.0, A4_W, 8.0, ACCENT);
        self.doc.text(MARGIN, A4_H - 50.0, 18.0, true, &r.title);
        self.doc.text(MARGIN, A4_H - 68.0, 10.0, false, &r.subtitle);
        self.doc.text(
            MARGIN,
            A4_H - 84.0,
            8.5,
            false,
            &format!(
                "Período: {} -> {}   |   Gerado em: {}",
                iso(r.period_start),
                iso(r.period_end),
                iso(r.generated_at)
            ),
        );
        self.doc
            .line(MARGIN, A4_H - 92.0, A4_W - MARGIN, A4_H - 92.0, 0.6, GREY);
        self.y = A4_H - 112.0;
    }

    fn footer(&mut self) {
        self.doc.line(MARGIN, 40.0, A4_W - MARGIN, 40.0, 0.4, GREY);
        self.doc.text(
            MARGIN,
            28.0,
            7.5,
            false,
            "Zylos Analytics — documento gerado automaticamente",
        );
        self.doc.text_right(
            A4_W - MARGIN,
            28.0,
            7.5,
            false,
            &format!("Página {}", self.page),
        );
    }

    fn ensure(&mut self, need: f64) -> bool {
        if self.y - need < BOTTOM {
            self.footer();
            self.doc.new_page();
            self.page += 1;
            self.header();
            true
        } else {
            false
        }
    }

    fn heading(&mut self, h: &str) {
        self.ensure(40.0);
        self.doc.text(MARGIN, self.y, 12.0, true, h);
        self.y -= 18.0;
    }

    fn table(&mut self, columns: &[String], rows: &[Vec<String>]) {
        let width = A4_W - 2.0 * MARGIN;
        let ncol = columns.len().max(1);
        // primeira coluna mais larga (rótulo); demais iguais
        let first = if ncol > 1 { width * 0.34 } else { width };
        let other = if ncol > 1 {
            (width - first) / (ncol - 1) as f64
        } else {
            0.0
        };
        let col_x = |i: usize| {
            if i == 0 {
                MARGIN
            } else {
                MARGIN + first + other * (i - 1) as f64
            }
        };
        let col_right = |i: usize| {
            if i == 0 {
                MARGIN + first
            } else {
                MARGIN + first + other * i as f64
            }
        };
        let max_chars = |i: usize| {
            let w = if i == 0 { first } else { other };
            ((w - 6.0) / (8.0 * 0.52)).max(3.0) as usize
        };
        let draw_head = |c: &mut Cursor<'_>| {
            c.doc.rect_fill(MARGIN, c.y - 4.0, width, 15.0, ACCENT);
            for (i, h) in columns.iter().enumerate() {
                let t = truncate(h, max_chars(i));
                if i == 0 {
                    c.doc.text(col_x(i) + 3.0, c.y, 8.0, true, &t);
                } else {
                    c.doc.text_right(col_right(i) - 3.0, c.y, 8.0, true, &t);
                }
            }
            // texto do cabeçalho em branco seria ideal; mantemos preto sobre azul claro
            c.y -= 15.0;
        };
        self.ensure(30.0);
        draw_head(self);
        for (ri, row) in rows.iter().enumerate() {
            if self.ensure(14.0) {
                draw_head(self);
            }
            if ri % 2 == 1 {
                self.doc.rect_fill(MARGIN, self.y - 4.0, width, 13.0, ZEBRA);
            }
            for (i, cell) in row.iter().enumerate().take(ncol) {
                let t = truncate(cell, max_chars(i));
                if i == 0 {
                    self.doc.text(col_x(i) + 3.0, self.y, 8.0, false, &t);
                } else {
                    self.doc
                        .text_right(col_right(i) - 3.0, self.y, 8.0, false, &t);
                }
            }
            self.y -= 13.0;
        }
        if rows.is_empty() {
            self.doc
                .text(MARGIN + 3.0, self.y, 8.0, false, "(sem dados no período)");
            self.y -= 13.0;
        }
        self.y -= 10.0;
    }

    fn bars(&mut self, unit: &str, items: &[(String, f64)]) {
        let max = items.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max);
        let label_w = 150.0;
        let bar_w = A4_W - 2.0 * MARGIN - label_w - 70.0;
        for (l, v) in items {
            self.ensure(14.0);
            self.doc.text(MARGIN, self.y, 8.0, false, &truncate(l, 34));
            let w = if max > 0.0 { bar_w * (v / max) } else { 0.0 };
            self.doc
                .rect_fill(MARGIN + label_w, self.y - 2.0, w.max(0.5), 9.0, ACCENT);
            self.doc.text(
                MARGIN + label_w + bar_w + 6.0,
                self.y,
                8.0,
                false,
                &format!("{} {unit}", fmt_num(*v)),
            );
            self.y -= 13.0;
        }
        if items.is_empty() {
            self.doc
                .text(MARGIN, self.y, 8.0, false, "(sem dados no período)");
            self.y -= 13.0;
        }
        self.y -= 10.0;
    }

    fn paragraph(&mut self, text: &str) {
        for line in wrap(text, 100) {
            self.ensure(13.0);
            self.doc.text(MARGIN, self.y, 9.0, false, &line);
            self.y -= 12.0;
        }
        self.y -= 8.0;
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 && v.abs() < 1e12 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Renderiza um [`Report`] como PDF A4 (bytes).
pub fn report_to_pdf(r: &Report) -> Vec<u8> {
    let mut c = Cursor {
        doc: PdfDoc::new(&r.title),
        y: 0.0,
        page: 1,
        report: r,
    };
    c.header();
    for s in &r.sections {
        match s {
            Section::Table {
                heading,
                columns,
                rows,
            } => {
                c.heading(heading);
                c.table(columns, rows);
            }
            Section::Bars {
                heading,
                unit,
                items,
            } => {
                c.heading(heading);
                c.bars(unit, items);
            }
            Section::Paragraph { heading, text } => {
                c.heading(heading);
                c.paragraph(text);
            }
        }
    }
    c.footer();
    c.doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escaping_and_injection_guard() {
        assert_eq!(csv_cell("a,b"), "\"a,b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("=HYPERLINK(1)"), "'=HYPERLINK(1)");
        assert_eq!(csv_cell("-3.5"), "-3.5");
    }

    #[test]
    fn events_csv_has_metadata_and_all_fields() {
        let e = AnalyticsEvent::new(1_794_787_200, "bot,1", "s", true, 42)
            .with_cost(0.5, 7)
            .with_dim("channel", "slack");
        let csv = events_to_csv(&[e], &Filter::all().agent("bot,1"), 1_794_787_200);
        let lines: Vec<&str> = csv.split("\r\n").collect();
        assert!(lines[0].starts_with("# Eventos"));
        assert!(lines[1].contains("agentes [bot,1]"));
        assert_eq!(lines[3], "# linhas: 1");
        assert_eq!(
            lines[4],
            "id,timestamp,agent,skill,success,latency_ms,cost_usd,tokens,dimensions"
        );
        assert!(lines[5].contains("2026-11-16T00:00:00Z,\"bot,1\",s,true,42,0.500000,7"));
        assert!(lines[5].contains("channel"));
    }

    #[test]
    fn wrap_and_truncate() {
        assert_eq!(wrap("a b c", 3), vec!["a b", "c"]);
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
