//! Cliente RFB (o protocolo do VNC) mínimo, só leitura: conecta a um servidor VNC, recebe o
//! framebuffer e entrega os quadros como imagem. É o que faltava para o `vnc_url` de uma sessão
//! deixar de ser só um texto guardado.
//!
//! **O que suporta** (de propósito pouco): RFB 3.3, 3.7 e 3.8; autenticação **nenhuma** (tipo 1);
//! formato de pixel 32 bpp truecolor pedido ao servidor; codificação **Raw**. Qualquer outra coisa
//! (autenticação por senha do VNC, Tight, ZRLE, Hextile...) vira um erro claro, nunca uma imagem
//! errada. Ele **não envia** mouse nem teclado: só lê.
//!
//! **Limites:** largura e altura até 8192, framebuffer até 64 MB, cada leitura com limite de
//! tempo, mensagens de texto do servidor até 1 MB. Um servidor que mande lixo ou gigantismo é
//! derrubado com erro, não consome memória sem fim.
//!
//! O cliente é genérico sobre `AsyncRead + AsyncWrite`: em produção é um `TcpStream`; nos testes é
//! um `tokio::io::duplex` com um servidor RFB falso do outro lado.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const LADO_MAX: u32 = 8192;
const FRAMEBUFFER_MAX: usize = 64 * 1024 * 1024;
const TEXTO_MAX: u32 = 1024 * 1024;
const TEMPO_LEITURA: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroRfb {
    /// Falha de rede ou tempo esgotado (texto curto, sem endereço).
    Rede(String),
    /// O servidor disse algo fora do protocolo ou grande demais.
    Protocolo(String),
    /// O servidor só oferece autenticação por senha (ou outra) e este cliente só fala "nenhuma".
    AutenticacaoNaoSuportada,
    /// O servidor mandou um retângulo numa codificação que não é Raw.
    CodificacaoNaoSuportada(i32),
    /// O servidor recusou a conexão e deu um motivo.
    Recusado(String),
}

impl std::fmt::Display for ErroRfb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rede(e) => write!(f, "falha de rede: {e}"),
            Self::Protocolo(e) => write!(f, "protocolo RFB inválido: {e}"),
            Self::AutenticacaoNaoSuportada => {
                write!(
                    f,
                    "o servidor VNC exige autenticação, que este cliente não suporta"
                )
            }
            Self::CodificacaoNaoSuportada(c) => {
                write!(f, "codificação RFB {c} não suportada (só Raw)")
            }
            Self::Recusado(m) => write!(f, "o servidor VNC recusou: {m}"),
        }
    }
}

impl std::error::Error for ErroRfb {}

/// Um quadro completo do framebuffer, em RGBA (8 bits por canal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quadro {
    pub largura: u32,
    pub altura: u32,
    pub rgba: Vec<u8>,
}

impl Quadro {
    /// O quadro como PNG.
    pub fn png(&self) -> Result<Vec<u8>, String> {
        let img = image::RgbaImage::from_raw(self.largura, self.altura, self.rgba.clone())
            .ok_or("tamanho do framebuffer inconsistente")?;
        crate::session::encode_png(&img)
    }
}

pub struct ClienteRfb<S> {
    stream: S,
    largura: u32,
    altura: u32,
    /// Framebuffer em RGBA, atualizado a cada retângulo.
    fb: Vec<u8>,
    pediu: bool,
}

async fn ler<S: AsyncRead + Unpin>(s: &mut S, buf: &mut [u8]) -> Result<(), ErroRfb> {
    match tokio::time::timeout(TEMPO_LEITURA, s.read_exact(buf)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(ErroRfb::Rede(e.kind().to_string())),
        Err(_) => Err(ErroRfb::Rede("tempo esgotado".into())),
    }
}

async fn ler_u8<S: AsyncRead + Unpin>(s: &mut S) -> Result<u8, ErroRfb> {
    let mut b = [0u8; 1];
    ler(s, &mut b).await?;
    Ok(b[0])
}

async fn ler_u16<S: AsyncRead + Unpin>(s: &mut S) -> Result<u16, ErroRfb> {
    let mut b = [0u8; 2];
    ler(s, &mut b).await?;
    Ok(u16::from_be_bytes(b))
}

async fn ler_u32<S: AsyncRead + Unpin>(s: &mut S) -> Result<u32, ErroRfb> {
    let mut b = [0u8; 4];
    ler(s, &mut b).await?;
    Ok(u32::from_be_bytes(b))
}

async fn descartar<S: AsyncRead + Unpin>(s: &mut S, mut n: usize) -> Result<(), ErroRfb> {
    let mut lixo = [0u8; 4096];
    while n > 0 {
        let k = n.min(lixo.len());
        ler(s, &mut lixo[..k]).await?;
        n -= k;
    }
    Ok(())
}

async fn enviar<S: AsyncWrite + Unpin>(s: &mut S, bytes: &[u8]) -> Result<(), ErroRfb> {
    s.write_all(bytes)
        .await
        .map_err(|e| ErroRfb::Rede(e.kind().to_string()))
}

impl<S: AsyncRead + AsyncWrite + Unpin> ClienteRfb<S> {
    /// Faz o aperto de mão até o `ServerInit` e pede 32 bpp truecolor com codificação Raw.
    pub async fn conectar(mut stream: S) -> Result<Self, ErroRfb> {
        // 1. Versão do protocolo: "RFB 003.00X\n".
        let mut versao = [0u8; 12];
        ler(&mut stream, &mut versao).await?;
        let v = std::str::from_utf8(&versao).map_err(|_| ErroRfb::Protocolo("versão".into()))?;
        let menor: u32 = v
            .strip_prefix("RFB 003.00")
            .and_then(|r| r.strip_suffix('\n'))
            .and_then(|r| r.parse().ok())
            .ok_or_else(|| ErroRfb::Protocolo("não é um servidor RFB".into()))?;
        // Responde com a maior versão que entendemos (3.3, 3.7 ou 3.8).
        let nossa = match menor {
            0..=6 => 3,
            7 => 7,
            _ => 8,
        };
        enviar(&mut stream, format!("RFB 003.00{nossa}\n").as_bytes()).await?;

        // 2. Segurança: só "nenhuma" (tipo 1).
        if nossa == 3 {
            // 3.3: o servidor decide e manda um u32.
            match ler_u32(&mut stream).await? {
                1 => {}
                0 => return Err(Self::razao(&mut stream).await),
                _ => return Err(ErroRfb::AutenticacaoNaoSuportada),
            }
        } else {
            let n = ler_u8(&mut stream).await?;
            if n == 0 {
                return Err(Self::razao(&mut stream).await);
            }
            let mut tipos = vec![0u8; usize::from(n)];
            ler(&mut stream, &mut tipos).await?;
            if !tipos.contains(&1) {
                return Err(ErroRfb::AutenticacaoNaoSuportada);
            }
            enviar(&mut stream, &[1]).await?;
            if nossa == 8 {
                // 3.8: SecurityResult (3.7 com "nenhuma" não manda).
                if ler_u32(&mut stream).await? != 0 {
                    return Err(Self::razao(&mut stream).await);
                }
            }
        }

        // 3. ClientInit (compartilhado) e ServerInit.
        enviar(&mut stream, &[1]).await?;
        let largura = u32::from(ler_u16(&mut stream).await?);
        let altura = u32::from(ler_u16(&mut stream).await?);
        let mut formato = [0u8; 16];
        ler(&mut stream, &mut formato).await?;
        let tam_nome = ler_u32(&mut stream).await?;
        if tam_nome > TEXTO_MAX {
            return Err(ErroRfb::Protocolo("nome do desktop grande demais".into()));
        }
        descartar(&mut stream, tam_nome as usize).await?;
        if largura == 0 || altura == 0 || largura > LADO_MAX || altura > LADO_MAX {
            return Err(ErroRfb::Protocolo(format!(
                "tela {largura}x{altura} fora do limite"
            )));
        }
        let bytes = (largura as usize) * (altura as usize) * 4;
        if bytes > FRAMEBUFFER_MAX {
            return Err(ErroRfb::Protocolo("framebuffer grande demais".into()));
        }

        // 4. SetPixelFormat (32 bpp, profundidade 24, little-endian, truecolor, R16 G8 B0) e
        //    SetEncodings (só Raw).
        let mut pf = vec![0u8, 0, 0, 0];
        pf.extend_from_slice(&[32, 24, 0, 1]);
        pf.extend_from_slice(&255u16.to_be_bytes());
        pf.extend_from_slice(&255u16.to_be_bytes());
        pf.extend_from_slice(&255u16.to_be_bytes());
        pf.extend_from_slice(&[16, 8, 0, 0, 0, 0]);
        enviar(&mut stream, &pf).await?;
        let mut enc = vec![2u8, 0];
        enc.extend_from_slice(&1u16.to_be_bytes());
        enc.extend_from_slice(&0i32.to_be_bytes());
        enviar(&mut stream, &enc).await?;

        Ok(Self {
            stream,
            largura,
            altura,
            // Começa opaco e preto: pixel nunca recebido não vira transparente.
            fb: [0u8, 0, 0, 255].repeat((largura as usize) * (altura as usize)),
            pediu: false,
        })
    }

    async fn razao(stream: &mut S) -> ErroRfb {
        let n = match ler_u32(stream).await {
            Ok(n) if n <= 4096 => n as usize,
            _ => return ErroRfb::Recusado("sem motivo".into()),
        };
        let mut b = vec![0u8; n];
        if ler(stream, &mut b).await.is_err() {
            return ErroRfb::Recusado("sem motivo".into());
        }
        ErroRfb::Recusado(String::from_utf8_lossy(&b).chars().take(120).collect())
    }

    pub fn tamanho(&self) -> (u32, u32) {
        (self.largura, self.altura)
    }

    /// Pede uma atualização e espera o próximo `FramebufferUpdate`, devolvendo o quadro inteiro.
    pub async fn proximo_quadro(&mut self) -> Result<Quadro, ErroRfb> {
        let mut req = vec![3u8, u8::from(self.pediu)];
        req.extend_from_slice(&0u16.to_be_bytes());
        req.extend_from_slice(&0u16.to_be_bytes());
        req.extend_from_slice(&(self.largura as u16).to_be_bytes());
        req.extend_from_slice(&(self.altura as u16).to_be_bytes());
        enviar(&mut self.stream, &req).await?;
        self.pediu = true;

        loop {
            match ler_u8(&mut self.stream).await? {
                0 => {
                    let _ = ler_u8(&mut self.stream).await?;
                    let n = ler_u16(&mut self.stream).await?;
                    for _ in 0..n {
                        self.retangulo().await?;
                    }
                    return Ok(Quadro {
                        largura: self.largura,
                        altura: self.altura,
                        rgba: self.fb.clone(),
                    });
                }
                // SetColourMapEntries: nada a fazer com truecolor; só consome.
                1 => {
                    let _ = ler_u8(&mut self.stream).await?;
                    let _primeira = ler_u16(&mut self.stream).await?;
                    let n = ler_u16(&mut self.stream).await?;
                    descartar(&mut self.stream, usize::from(n) * 6).await?;
                }
                // Bell.
                2 => {}
                // ServerCutText.
                3 => {
                    descartar(&mut self.stream, 3).await?;
                    let n = ler_u32(&mut self.stream).await?;
                    if n > TEXTO_MAX {
                        return Err(ErroRfb::Protocolo("texto do servidor grande demais".into()));
                    }
                    descartar(&mut self.stream, n as usize).await?;
                }
                t => return Err(ErroRfb::Protocolo(format!("mensagem {t} desconhecida"))),
            }
        }
    }

    async fn retangulo(&mut self) -> Result<(), ErroRfb> {
        let x = u32::from(ler_u16(&mut self.stream).await?);
        let y = u32::from(ler_u16(&mut self.stream).await?);
        let w = u32::from(ler_u16(&mut self.stream).await?);
        let h = u32::from(ler_u16(&mut self.stream).await?);
        let enc = ler_u32(&mut self.stream).await? as i32;
        if enc != 0 {
            return Err(ErroRfb::CodificacaoNaoSuportada(enc));
        }
        if x + w > self.largura || y + h > self.altura {
            return Err(ErroRfb::Protocolo("retângulo fora da tela".into()));
        }
        let mut linha = vec![0u8; (w as usize) * 4];
        for j in 0..h {
            ler(&mut self.stream, &mut linha).await?;
            let base = (((y + j) * self.largura + x) as usize) * 4;
            for i in 0..w as usize {
                // O servidor manda B, G, R, (nada): little-endian com R no byte 2.
                let (b, g, r) = (linha[i * 4], linha[i * 4 + 1], linha[i * 4 + 2]);
                let d = base + i * 4;
                self.fb[d] = r;
                self.fb[d + 1] = g;
                self.fb[d + 2] = b;
                self.fb[d + 3] = 255;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use tokio::io::duplex;

    /// Um retângulo do servidor falso: `(x, y, w, h, codificação, bytes B,G,R,X)`.
    type Retangulo = (u16, u16, u16, u16, i32, Vec<u8>);

    /// Servidor RFB falso: faz o aperto de mão e manda os `quadros` (cada um uma lista de
    /// retângulos `(x, y, w, h, codificação, bytes B,G,R,X)`), um por pedido do cliente.
    async fn servidor(
        mut s: tokio::io::DuplexStream,
        versao: &'static str,
        tipos: Vec<u8>,
        tela: (u16, u16),
        quadros: Vec<Vec<Retangulo>>,
    ) {
        s.write_all(format!("RFB 003.00{versao}\n").as_bytes())
            .await
            .unwrap();
        let mut resp = [0u8; 12];
        s.read_exact(&mut resp).await.unwrap();
        if versao == "3" {
            s.write_all(&1u32.to_be_bytes()).await.unwrap();
        } else {
            s.write_all(&[tipos.len() as u8]).await.unwrap();
            s.write_all(&tipos).await.unwrap();
            if !tipos.contains(&1) {
                return;
            }
            let mut escolha = [0u8; 1];
            s.read_exact(&mut escolha).await.unwrap();
            if versao == "8" {
                s.write_all(&0u32.to_be_bytes()).await.unwrap();
            }
        }
        let mut shared = [0u8; 1];
        s.read_exact(&mut shared).await.unwrap();
        s.write_all(&tela.0.to_be_bytes()).await.unwrap();
        s.write_all(&tela.1.to_be_bytes()).await.unwrap();
        s.write_all(&[32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0])
            .await
            .unwrap();
        s.write_all(&4u32.to_be_bytes()).await.unwrap();
        s.write_all(b"fake").await.unwrap();
        // SetPixelFormat (20 bytes) e SetEncodings (8 bytes).
        let mut lixo = [0u8; 28];
        s.read_exact(&mut lixo).await.unwrap();
        for q in quadros {
            let mut req = [0u8; 10];
            if s.read_exact(&mut req).await.is_err() {
                return;
            }
            // Antes do FramebufferUpdate, um Bell e um ServerCutText: o cliente tem de pular.
            s.write_all(&[2]).await.unwrap();
            s.write_all(&[3, 0, 0, 0]).await.unwrap();
            s.write_all(&3u32.to_be_bytes()).await.unwrap();
            s.write_all(b"abc").await.unwrap();
            s.write_all(&[0, 0]).await.unwrap();
            s.write_all(&(q.len() as u16).to_be_bytes()).await.unwrap();
            for (x, y, w, h, enc, dados) in q {
                s.write_all(&x.to_be_bytes()).await.unwrap();
                s.write_all(&y.to_be_bytes()).await.unwrap();
                s.write_all(&w.to_be_bytes()).await.unwrap();
                s.write_all(&h.to_be_bytes()).await.unwrap();
                s.write_all(&enc.to_be_bytes()).await.unwrap();
                s.write_all(&dados).await.unwrap();
            }
        }
    }

    fn pixels(w: u16, h: u16, bgrx: [u8; 4]) -> Vec<u8> {
        bgrx.repeat(usize::from(w) * usize::from(h))
    }

    #[tokio::test]
    async fn le_a_tela_inteira_e_atualiza_so_o_retangulo_novo() {
        for versao in ["8", "7", "3"] {
            let (c, s) = duplex(1 << 16);
            let srv = tokio::spawn(servidor(
                s,
                versao,
                vec![1],
                (4, 3),
                vec![
                    // Quadro 1: tela toda vermelha (B=0,G=0,R=255).
                    vec![(0, 0, 4, 3, 0, pixels(4, 3, [0, 0, 255, 0]))],
                    // Quadro 2: só um retângulo 2x1 em (1,1) azul (B=255,G=0,R=0).
                    vec![(1, 1, 2, 1, 0, pixels(2, 1, [255, 0, 0, 0]))],
                ],
            ));
            let mut cli = ClienteRfb::conectar(c).await.unwrap();
            assert_eq!(cli.tamanho(), (4, 3), "versão {versao}");

            let q1 = cli.proximo_quadro().await.unwrap();
            assert_eq!((q1.largura, q1.altura), (4, 3));
            assert!(
                q1.rgba.chunks(4).all(|p| p == [255, 0, 0, 255]),
                "tudo vermelho"
            );

            let q2 = cli.proximo_quadro().await.unwrap();
            let px = |x: usize, y: usize| q2.rgba[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4].to_vec();
            assert_eq!(px(1, 1), [0, 0, 255, 255], "retângulo novo, azul");
            assert_eq!(px(2, 1), [0, 0, 255, 255]);
            assert_eq!(
                px(0, 1),
                [255, 0, 0, 255],
                "fora do retângulo segue vermelho"
            );
            assert_eq!(px(3, 2), [255, 0, 0, 255]);
            // O PNG do quadro tem a assinatura PNG.
            assert_eq!(&q2.png().unwrap()[..4], &[0x89, b'P', b'N', b'G']);
            srv.await.unwrap();
        }
    }

    #[tokio::test]
    async fn so_senha_do_vnc_e_codificacao_que_nao_e_raw_viram_erro_claro() {
        // Servidor que só oferece autenticação VNC (tipo 2): este cliente não fala isso.
        let (c, s) = duplex(1 << 16);
        let srv = tokio::spawn(servidor(s, "8", vec![2], (4, 3), vec![]));
        assert_eq!(
            ClienteRfb::conectar(c).await.err(),
            Some(ErroRfb::AutenticacaoNaoSuportada)
        );
        srv.await.unwrap();

        // Servidor que responde com Tight (7): erro, não uma imagem errada.
        let (c, s) = duplex(1 << 16);
        let srv = tokio::spawn(servidor(
            s,
            "8",
            vec![1],
            (4, 3),
            vec![vec![(0, 0, 1, 1, 7, vec![0; 4])]],
        ));
        let mut cli = ClienteRfb::conectar(c).await.unwrap();
        assert_eq!(
            cli.proximo_quadro().await.err(),
            Some(ErroRfb::CodificacaoNaoSuportada(7))
        );
        drop(cli);
        let _ = srv.await;
    }

    #[tokio::test]
    async fn tela_gigante_retangulo_fora_e_lixo_sao_recusados() {
        // Tela maior que o limite.
        let (c, s) = duplex(1 << 16);
        let srv = tokio::spawn(servidor(s, "8", vec![1], (9000, 100), vec![]));
        assert!(matches!(
            ClienteRfb::conectar(c).await.err(),
            Some(ErroRfb::Protocolo(_))
        ));
        let _ = srv.await;

        // Retângulo que passa da tela.
        let (c, s) = duplex(1 << 16);
        let srv = tokio::spawn(servidor(
            s,
            "8",
            vec![1],
            (4, 3),
            vec![vec![(3, 0, 4, 1, 0, pixels(4, 1, [0; 4]))]],
        ));
        let mut cli = ClienteRfb::conectar(c).await.unwrap();
        assert!(matches!(
            cli.proximo_quadro().await.err(),
            Some(ErroRfb::Protocolo(_))
        ));
        drop(cli);
        let _ = srv.await;

        // Não é um servidor RFB.
        let (c, mut s) = duplex(1 << 16);
        tokio::spawn(async move {
            let _ = s.write_all(b"HTTP/1.1 400\r\n").await;
        });
        assert!(matches!(
            ClienteRfb::conectar(c).await.err(),
            Some(ErroRfb::Protocolo(_))
        ));
    }
}
