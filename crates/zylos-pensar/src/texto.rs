//! Leitura de arquivos de texto escritos à mão no Windows: tira BOM UTF-8,
//! recusa UTF-16 (o que o PowerShell 5.1 grava por padrão) e texto que não é UTF-8.

use std::path::Path;

/// Erro ao ler um arquivo de texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroTexto {
    Inexistente,
    Ilegivel(String),
    Utf16,
    NaoUtf8,
    Grande { limite: usize },
}

impl std::fmt::Display for ErroTexto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErroTexto::Inexistente => write!(f, "arquivo não existe"),
            ErroTexto::Ilegivel(e) => write!(f, "não deu para ler o arquivo: {e}"),
            ErroTexto::Utf16 => write!(f, "arquivo está em UTF-16; salve como UTF-8"),
            ErroTexto::NaoUtf8 => write!(f, "arquivo não está em UTF-8"),
            ErroTexto::Grande { limite } => write!(f, "arquivo passa do limite de {limite} bytes"),
        }
    }
}

/// Lê `caminho` como UTF-8, sem BOM, com limite opcional de tamanho.
pub fn ler(caminho: &Path, limite: Option<usize>) -> Result<String, ErroTexto> {
    let bytes = match std::fs::read(caminho) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ErroTexto::Inexistente),
        Err(e) => return Err(ErroTexto::Ilegivel(e.kind().to_string())),
    };
    if let Some(l) = limite {
        if bytes.len() > l {
            return Err(ErroTexto::Grande { limite: l });
        }
    }
    decodificar(bytes)
}

/// Decodifica bytes como UTF-8, tirando o BOM e recusando UTF-16.
pub fn decodificar(bytes: Vec<u8>) -> Result<String, ErroTexto> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(ErroTexto::Utf16);
    }
    let sem_bom = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { bytes[3..].to_vec() } else { bytes };
    String::from_utf8(sem_bom).map_err(|_| ErroTexto::NaoUtf8)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tira_bom_e_recusa_utf16() {
        assert_eq!(decodificar(b"\xEF\xBB\xBFoi".to_vec()).unwrap(), "oi");
        assert_eq!(decodificar(b"\xFF\xFEo\0i\0".to_vec()), Err(ErroTexto::Utf16));
        assert_eq!(decodificar(vec![0xC3, 0x28]), Err(ErroTexto::NaoUtf8));
    }

    #[test]
    fn limite_de_tamanho() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.md");
        std::fs::write(&p, "12345").unwrap();
        assert_eq!(ler(&p, Some(4)), Err(ErroTexto::Grande { limite: 4 }));
        assert_eq!(ler(&p, Some(5)).unwrap(), "12345");
        assert_eq!(ler(&d.path().join("x"), None), Err(ErroTexto::Inexistente));
    }
}
