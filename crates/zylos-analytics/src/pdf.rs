//! Writer PDF 1.4 mínimo, sem dependências (T11.10.4).
//!
//! ## Por que um writer próprio (e não `printpdf`/`genpdf`)?
//! - Os relatórios só precisam de texto, linhas e retângulos preenchidos
//!   (tabelas + barras simples). As 14 fontes padrão do PDF (Helvetica)
//!   dispensam embutir fontes, então o arquivo inteiro é ASCII + streams.
//! - `printpdf` 0.7+ puxa `lopdf`, `image`, `owned_ttf_parser` etc. — dezenas
//!   de crates transitivos para uma funcionalidade de ~250 linhas, o que
//!   aumenta superfície de supply-chain (`cargo deny`/`cargo audit`) e tempo
//!   de build do workspace.
//! - Saída determinística (sem timestamps/IDs aleatórios) → testável por
//!   snapshot e reproduzível.
//!
//! Limitações conhecidas: sem compressão de streams, sem fontes TrueType,
//! acentuação limitada ao subconjunto Latin-1 do `WinAnsiEncoding` (cobre
//! pt-BR). Caracteres fora disso viram `?`.
//!
//! Página A4 retrato: 595 × 842 pt.

/// Largura A4 em pontos.
pub const A4_W: f64 = 595.0;
/// Altura A4 em pontos.
pub const A4_H: f64 = 842.0;

/// Documento PDF em construção.
#[derive(Debug, Default)]
pub struct PdfDoc {
    pages: Vec<String>,
    current: String,
    title: String,
}

/// Cor RGB 0..1.
pub type Rgb = (f64, f64, f64);

impl PdfDoc {
    /// Novo documento com uma página vazia.
    pub fn new(title: &str) -> Self {
        Self {
            pages: Vec::new(),
            current: String::new(),
            title: title.to_string(),
        }
    }

    /// Fecha a página atual e inicia uma nova.
    pub fn new_page(&mut self) {
        let done = std::mem::take(&mut self.current);
        self.pages.push(done);
    }

    /// Número de páginas (incluindo a atual).
    pub fn page_count(&self) -> usize {
        self.pages.len() + 1
    }

    /// Escreve texto em (x, y) — origem no canto inferior esquerdo.
    pub fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, s: &str) {
        let font = if bold { "F2" } else { "F1" };
        self.current.push_str(&format!(
            "BT /{font} {size:.1} Tf {x:.2} {y:.2} Td ({}) Tj ET\n",
            escape(s)
        ));
    }

    /// Escreve texto alinhado à direita terminando em `x_right`.
    pub fn text_right(&mut self, x_right: f64, y: f64, size: f64, bold: bool, s: &str) {
        let w = text_width(s, size);
        self.text(x_right - w, y, size, bold, s);
    }

    /// Linha de (x1,y1) a (x2,y2).
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64, c: Rgb) {
        self.current.push_str(&format!(
            "{:.3} {:.3} {:.3} RG {width:.2} w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n",
            c.0, c.1, c.2
        ));
    }

    /// Retângulo preenchido.
    pub fn rect_fill(&mut self, x: f64, y: f64, w: f64, h: f64, c: Rgb) {
        self.current.push_str(&format!(
            "{:.3} {:.3} {:.3} rg {x:.2} {y:.2} {w:.2} {h:.2} re f 0 0 0 rg\n",
            c.0, c.1, c.2
        ));
    }

    /// Serializa o documento para bytes PDF.
    pub fn finish(mut self) -> Vec<u8> {
        self.new_page();
        let pages = std::mem::take(&mut self.pages);
        let n = pages.len();
        // Objetos: 1 Catalog, 2 Pages, 3 F1, 4 F2, 5 Info, depois (page, content) por página
        let mut objs: Vec<String> = Vec::new();
        objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 6 + i * 2)).collect();
        objs.push(format!(
            "<< /Type /Pages /Kids [{}] /Count {n} >>",
            kids.join(" ")
        ));
        objs.push(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .into(),
        );
        objs.push(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .into(),
        );
        objs.push(format!(
            "<< /Title ({}) /Producer (zylos-analytics) >>",
            escape(&self.title)
        ));
        for (i, content) in pages.iter().enumerate() {
            let content_id = 7 + i * 2;
            objs.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {A4_W} {A4_H}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_id} 0 R >>"
            ));
            let bytes = encode(content);
            objs.push(format!(
                "<< /Length {} >>\nstream\n{}\nendstream",
                bytes.len(),
                // conteúdo já é ASCII + bytes Latin-1; mantemos como String via latin1
                latin1_to_string(&bytes)
            ));
        }
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(objs.len());
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(&encode(o));
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

/// Escapa string literal PDF (`(`, `)`, `\`) e normaliza para Latin-1.
fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                o.push('\\');
                o.push(c);
            }
            '\n' | '\r' | '\t' => o.push(' '),
            '—' | '–' => o.push('-'),
            '≥' => o.push_str(">="),
            '≤' => o.push_str("<="),
            '→' => o.push_str("->"),
            c if (c as u32) < 0x20 => o.push(' '),
            c if (c as u32) <= 0xFF => o.push(c),
            _ => o.push('?'),
        }
    }
    o
}

/// Codifica para bytes Latin-1 (chars > 0xFF já foram trocados por `escape`).
fn encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if (c as u32) <= 0xFF {
                c as u32 as u8
            } else {
                b'?'
            }
        })
        .collect()
}

fn latin1_to_string(b: &[u8]) -> String {
    b.iter().map(|&x| x as char).collect()
}

/// Largura aproximada de texto Helvetica (média 0,52 em) — usada para alinhamento.
pub fn text_width(s: &str, size: f64) -> f64 {
    s.chars().count() as f64 * size * 0.52
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_valid_structure() {
        let mut d = PdfDoc::new("Relatório (teste)");
        d.text(50.0, 800.0, 12.0, true, "Olá — ação (x)");
        d.rect_fill(50.0, 700.0, 100.0, 10.0, (0.2, 0.4, 0.8));
        d.new_page();
        d.text(50.0, 800.0, 10.0, false, "página 2");
        assert_eq!(d.page_count(), 2);
        let bytes = d.finish();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("/Count 2"));
        assert!(s.contains("/MediaBox [0 0 595 842]"));
        assert!(s.contains("\\(x\\)"));
    }

    fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
        h[from..]
            .windows(n.len())
            .position(|w| w == n)
            .map(|p| p + from)
    }

    #[test]
    fn xref_offsets_point_to_objects() {
        let mut d = PdfDoc::new("t");
        d.text(10.0, 10.0, 10.0, false, "ação");
        let b = d.finish();
        let sx = b.windows(10).rposition(|w| w == b"startxref\n").unwrap();
        let tail = String::from_utf8_lossy(&b[sx + 10..]).to_string();
        let xref_off: usize = tail.lines().next().unwrap().trim().parse().unwrap();
        assert!(b[xref_off..].starts_with(b"xref"));
        let table = String::from_utf8_lossy(&b[xref_off..]).to_string();
        let entries: Vec<&str> = table.lines().skip(3).take(5).collect();
        for (i, e) in entries.iter().enumerate() {
            let off: usize = e[..10].parse().unwrap();
            assert!(
                b[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()),
                "obj {}",
                i + 1
            );
        }
    }

    #[test]
    fn stream_length_matches() {
        let mut d = PdfDoc::new("t");
        d.text(10.0, 10.0, 10.0, false, "çãé");
        let b = d.finish();
        let li = find(&b, b"/Length ", 0).unwrap();
        let num = String::from_utf8_lossy(&b[li + 8..li + 20]).to_string();
        let len: usize = num.split_whitespace().next().unwrap().parse().unwrap();
        let st = find(&b, b"stream\n", li).unwrap() + 7;
        let en = find(&b, b"\nendstream", st).unwrap();
        assert_eq!(en - st, len);
    }
}
