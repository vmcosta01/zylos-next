//! Entrada remota: cliques e teclas que chegam pelo WebSocket do relay e viram ação no computador
//! onde o gateway roda. **Controla a máquina de quem roda o gateway**, por isso tem várias travas,
//! todas obrigatórias:
//!
//! 1. **Desligada por padrão.** Só liga com `ZYLOS_COMPUTER_VIEW_INPUT=1` e, mesmo assim, só se o
//!    gateway foi compilado com `--features input-injection` (sem isso não existe injetor real).
//! 2. **Só com controle humano ativo, e só do dono dele.** A sessão precisa estar em `user_control`
//!    com a concessão (lease) ainda válida **e** o evento precisa vir de uma conexão autenticada cujo
//!    usuário (`sub` do JWT, verificado pelo gateway) é o dono da concessão. Sem login ou de outro
//!    usuário, nada é injetado.
//! 3. **Só o que for explicitamente permitido:** botões `left`/`right`/`middle`; teclas de uma lista
//!    curta (letras, números, pontuação comum, Enter, Backspace, Tab, Esc, setas, Home/End,
//!    PageUp/PageDown, Delete, Shift, Control, Alt). A tecla Windows/Meta e qualquer nome fora da
//!    lista são recusados.
//! 4. **Coordenadas limitadas** (0 a 16384) e **taxa limitada**: 60 eventos por segundo no total,
//!    somando todas as conexões (a máquina tem um mouse e um teclado só).
//! 5. **Um dono por vez na máquina.** Duas sessões com controle ao mesmo tempo não intercalam
//!    teclas: quem injetou por último segura a máquina enquanto o controle dele valer e ele tiver
//!    mandado algo nos últimos [`POSSE_OCIOSA`]. Quando o dono muda, as teclas que o anterior deixou
//!    apertadas são soltas antes do primeiro evento do novo.
//! 6. **Atalhos que fogem da janela estão fora**: Ctrl+Esc, Ctrl+Shift+Esc, Alt+Esc e Alt+Tab
//!    (menu Iniciar, Gerenciador de Tarefas, troca de janela), em qualquer ordem de aperto.
//!
//! O injetor é um trait: os testes usam um falso que só anota o que receberia, então nenhum teste
//! mexe no mouse ou no teclado de verdade.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zylos_computer_view::schema::SessionState;
use zylos_computer_view::session::ComputerSessionManager;

/// Variável que liga a entrada remota (padrão: desligada).
pub const ENTRADA_ENV: &str = "ZYLOS_COMPUTER_VIEW_INPUT";
/// Maior coordenada aceita (px do quadro).
pub const COORD_MAX: i32 = 16_384;
/// Eventos de entrada por segundo (somando todas as conexões: o computador tem um teclado só).
pub const EVENTOS_POR_SEGUNDO: usize = 60;
/// Depois de quanto tempo sem eventos o dono da máquina deixa outra sessão assumir a injeção.
pub const POSSE_OCIOSA: Duration = Duration::from_secs(30);

/// O que fazer com a entrada remota, dadas as três travas (variável, captura local, feature).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Porta {
    /// Ninguém pediu a entrada: segue desligada, sem aviso.
    Desligada,
    /// Pedida, mas a captura local está desligada: os quadros viriam de outro lugar.
    SemCaptura,
    /// Pedida, mas o gateway foi compilado sem o injetor real.
    SemInjetor,
    Ligar,
}

/// Decide se a entrada liga. Só liga com as TRÊS travas: `ZYLOS_COMPUTER_VIEW_INPUT=1`,
/// `ZYLOS_COMPUTER_VIEW_CAPTURE=1` e o gateway compilado com `--features input-injection`.
pub fn decidir_porta(pedida: bool, captura_local: bool, injetor_compilado: bool) -> Porta {
    match (pedida, captura_local, injetor_compilado) {
        (false, _, _) => Porta::Desligada,
        (true, false, _) => Porta::SemCaptura,
        (true, true, false) => Porta::SemInjetor,
        (true, true, true) => Porta::Ligar,
    }
}

/// Só o valor `1` (com ou sem espaços ao redor) liga; ausente ou qualquer outra coisa, não.
pub fn ligada_valor(v: Option<&str>) -> bool {
    v.is_some_and(|v| v.trim() == "1")
}

pub fn ligada_por_env() -> bool {
    ligada_valor(std::env::var(ENTRADA_ENV).ok().as_deref())
}

/// O dono da concessão de controle é o par tenant + usuário (`tenant:sub`), não só o `sub`: o mesmo
/// `sub` em outro tenant é outra pessoa. É o valor gravado em `ControlLease.user_id` (no login, pelo
/// middleware) e o que o relay passa a [`Entrada`] a cada evento.
pub fn dono_da_concessao(c: &zylos_auth::Claims) -> String {
    format!("{}:{}", c.tenant_id, c.sub)
}

/// Só administrador usa a entrada remota (o mesmo papel exigido para assumir o controle). Sem login
/// (`None`) não há o que conferir aqui: a [`Entrada`] já recusa quem não tem usuário.
pub fn pode_injetar(c: Option<&zylos_auth::Claims>) -> bool {
    c.is_none_or(crate::api_auth::eh_admin)
}

/// Quem de fato mexe no mouse e no teclado.
pub trait InjetorEntrada: Send + Sync {
    fn clique(&self, x: i32, y: i32, botao: &str) -> Result<(), String>;
    fn tecla(&self, tecla: &str, apertada: bool) -> Result<(), String>;
}

/// Por que um evento foi recusado (texto curto, vai para o cockpit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recusa {
    SessaoSemControle,
    /// Conexão sem login, ou de um usuário que não é o dono da concessão.
    OutroUsuario,
    BotaoInvalido,
    TeclaNaoPermitida,
    CoordenadaInvalida,
    /// A tela da sessão vem de outro computador (VNC): clicar aqui acertaria a máquina errada.
    SessaoRemota,
    /// Outra sessão está com o mouse e o teclado desta máquina agora.
    MaquinaOcupada,
    /// Mais de 60 eventos por segundo na máquina.
    EventosDemais,
    /// Ctrl+Esc, Alt+Tab e parentes: abrem o menu Iniciar, o Gerenciador de Tarefas, a troca de janela.
    AtalhoBloqueado,
    Falha(String),
}

impl Recusa {
    pub fn texto(&self) -> String {
        match self {
            Self::SessaoSemControle => {
                "Assuma o controle da sessão antes de enviar cliques e teclas".into()
            }
            Self::OutroUsuario => {
                "O controle desta sessão é de outra pessoa (ou você não está logado)".into()
            }
            Self::BotaoInvalido => "Botão do mouse não permitido".into(),
            Self::TeclaNaoPermitida => "Tecla não permitida".into(),
            Self::CoordenadaInvalida => "Coordenada fora do limite".into(),
            Self::SessaoRemota => {
                "Esta sessão mostra a tela de outro computador (VNC): a entrada remota só vale para a tela deste computador".into()
            }
            Self::MaquinaOcupada => {
                "Outra sessão está usando o mouse e o teclado deste computador agora".into()
            }
            Self::EventosDemais => "Eventos demais por segundo".into(),
            Self::AtalhoBloqueado => {
                "Atalho bloqueado (Ctrl+Esc, Alt+Esc, Alt+Tab): abre menus do sistema".into()
            }
            Self::Falha(e) => format!("Falha ao enviar a entrada: {e}"),
        }
    }
}

/// A tecla de espaço, chamada pelo nome para não depender de um `" "` que some num diff.
pub const TECLA_ESPACO: &str = " ";

/// Teclas aceitas: um caractere visível (letra, número, pontuação comum) ou um nome da lista.
pub fn tecla_permitida(code: &str) -> bool {
    const NOMES: &[&str] = &[
        "Enter",
        "Backspace",
        "Tab",
        "Escape",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Delete",
        "Shift",
        "Control",
        "Alt",
        TECLA_ESPACO,
    ];
    if NOMES.contains(&code) {
        return true;
    }
    let mut cs = code.chars();
    match (cs.next(), cs.next()) {
        (Some(c), None) => c.is_ascii_graphic(),
        _ => false,
    }
}

pub fn botao_valido(b: &str) -> bool {
    matches!(b, "left" | "right" | "middle")
}

/// A porta de entrada: confere as travas e só então chama o injetor.
pub struct Entrada {
    injetor: Arc<dyn InjetorEntrada>,
    manager: Arc<ComputerSessionManager>,
    estado: Mutex<Estado>,
}

/// O que vale para a máquina inteira (não para uma conexão): quem manda, o que está apertado, o ritmo.
struct Estado {
    /// (sessão, usuário) que injetou por último.
    dono: Option<(String, String)>,
    ultimo_evento: Option<Instant>,
    /// Teclas apertadas e ainda não soltas, de qualquer conexão.
    apertadas: Vec<String>,
    limite: LimiteDeTaxa,
}

/// `true` se apertar `tecla` agora, com `apertadas` já seguradas, abriria um atalho do sistema.
fn forma_atalho_bloqueado(apertadas: &[String], tecla: &str) -> bool {
    let tem = |t: &str| apertadas.iter().any(|a| a == t);
    match tecla {
        "Escape" => tem("Control") || tem("Alt"),
        "Tab" => tem("Alt"),
        // Na ordem inversa (Esc/Tab já seguradas e depois o modificador) o resultado é o mesmo.
        "Control" => tem("Escape"),
        "Alt" => tem("Escape") || tem("Tab"),
        _ => false,
    }
}

impl Entrada {
    pub fn new(injetor: Arc<dyn InjetorEntrada>, manager: Arc<ComputerSessionManager>) -> Self {
        Self {
            injetor,
            manager,
            estado: Mutex::new(Estado {
                dono: None,
                ultimo_evento: None,
                apertadas: Vec::new(),
                limite: LimiteDeTaxa::new(),
            }),
        }
    }

    /// Trava o estado da máquina. Se alguma thread entrou em pânico segurando a trava, seguimos com
    /// o que ficou (`into_inner`) em vez de propagar o pânico: o estado é só "quem manda, o que está
    /// apertado, o ritmo", e derrubar o gateway inteiro (ou deixar a entrada travada com teclas
    /// apertadas) é pior que um dono ou contador momentaneamente desatualizado.
    fn estado(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A sessão ainda tem controle humano válido (concessão não vencida)?
    fn controle_vivo(&self, session_id: &str) -> bool {
        self.manager.get_session(session_id).is_some_and(|s| {
            s.state == SessionState::UserControl.as_str()
                && s.control_lease.is_some_and(|l| !l.is_expired())
        })
    }

    /// Depois de `autorizar`: garante que esta sessão/usuário é o dono da máquina (ou que o dono
    /// anterior largou), solta o que o dono anterior deixou apertado e aplica o limite de taxa.
    fn tomar_a_vez(&self, session_id: &str, usuario: Option<&str>) -> Result<(), Recusa> {
        let u = usuario.unwrap_or_default();
        let mut st = self.estado();
        let sou_eu = st
            .dono
            .as_ref()
            .is_some_and(|(s, uu)| s == session_id && uu == u);
        if !sou_eu {
            if let Some((outra, _)) = &st.dono {
                let ativo = st.ultimo_evento.is_some_and(|t| t.elapsed() < POSSE_OCIOSA);
                if outra != session_id && ativo && self.controle_vivo(outra) {
                    return Err(Recusa::MaquinaOcupada);
                }
            }
            // Troca de dono: nada do anterior pode sobrar apertado (viraria Ctrl+C do novo).
            for t in std::mem::take(&mut st.apertadas) {
                let _ = self.injetor.tecla(&t, false);
            }
            st.dono = Some((session_id.to_string(), u.to_string()));
        }
        if !st.limite.permite() {
            return Err(Recusa::EventosDemais);
        }
        st.ultimo_evento = Some(Instant::now());
        Ok(())
    }

    /// A sessão tem controle humano ativo, com a concessão ainda válida, e `usuario` (o `sub` do
    /// JWT da conexão, `None` se não autenticada) é o dono dela?
    pub fn autorizar(&self, session_id: &str, usuario: Option<&str>) -> Result<(), Recusa> {
        let sessao = self
            .manager
            .get_session(session_id)
            .filter(|s| s.state == SessionState::UserControl.as_str())
            .ok_or(Recusa::SessaoSemControle)?;
        // A entrada vai para o computador do gateway; numa sessão VNC a tela é de outro.
        if sessao.vnc_url.is_some() {
            return Err(Recusa::SessaoRemota);
        }
        let lease = sessao
            .control_lease
            .filter(|l| !l.is_expired())
            .ok_or(Recusa::SessaoSemControle)?;
        match usuario {
            Some(u) if u == lease.user_id => Ok(()),
            _ => Err(Recusa::OutroUsuario),
        }
    }

    pub fn clique(
        &self,
        session_id: &str,
        usuario: Option<&str>,
        x: i32,
        y: i32,
        botao: &str,
    ) -> Result<(), Recusa> {
        self.autorizar(session_id, usuario)?;
        if !botao_valido(botao) {
            return Err(Recusa::BotaoInvalido);
        }
        if !(0..=COORD_MAX).contains(&x) || !(0..=COORD_MAX).contains(&y) {
            return Err(Recusa::CoordenadaInvalida);
        }
        self.tomar_a_vez(session_id, usuario)?;
        self.injetor.clique(x, y, botao).map_err(Recusa::Falha)
    }

    pub fn tecla(
        &self,
        session_id: &str,
        usuario: Option<&str>,
        tecla: &str,
        apertada: bool,
    ) -> Result<(), Recusa> {
        self.autorizar(session_id, usuario)?;
        if !tecla_permitida(tecla) {
            return Err(Recusa::TeclaNaoPermitida);
        }
        self.tomar_a_vez(session_id, usuario)?;
        let mut st = self.estado();
        if apertada && forma_atalho_bloqueado(&st.apertadas, tecla) {
            return Err(Recusa::AtalhoBloqueado);
        }
        self.injetor.tecla(tecla, apertada).map_err(Recusa::Falha)?;
        if apertada {
            if !st.apertadas.iter().any(|a| a == tecla) {
                st.apertadas.push(tecla.to_string());
            }
        } else {
            st.apertadas.retain(|a| a != tecla);
        }
        Ok(())
    }

    /// Solta uma tecla que esta conexão apertou. **Não exige controle nem conta na taxa**: perder o
    /// controle (ou estourar o limite) com uma tecla apertada a deixaria presa no sistema.
    pub fn soltar(&self, tecla: &str) -> Result<(), Recusa> {
        if !tecla_permitida(tecla) {
            return Err(Recusa::TeclaNaoPermitida);
        }
        self.estado().apertadas.retain(|a| a != tecla);
        self.injetor.tecla(tecla, false).map_err(Recusa::Falha)
    }
}

/// Limite de eventos por segundo (janela deslizante: em QUALQUER intervalo de 1 s cabem no máximo
/// [`EVENTOS_POR_SEGUNDO`]; a janela fixa deixava passar o dobro na virada).
pub struct LimiteDeTaxa {
    /// Instantes dos eventos aceitos na última janela, do mais antigo ao mais novo.
    aceitos: std::collections::VecDeque<Instant>,
}

impl Default for LimiteDeTaxa {
    fn default() -> Self {
        Self::new()
    }
}

impl LimiteDeTaxa {
    pub fn new() -> Self {
        Self {
            aceitos: std::collections::VecDeque::with_capacity(EVENTOS_POR_SEGUNDO),
        }
    }

    /// `true` se este evento cabe na janela atual.
    pub fn permite(&mut self) -> bool {
        self.permite_em(Instant::now())
    }

    /// Como [`Self::permite`], com o instante dado (para testar sem dormir). Evento recusado não
    /// ocupa vaga na janela.
    fn permite_em(&mut self, agora: Instant) -> bool {
        let janela = Duration::from_secs(1);
        while self
            .aceitos
            .front()
            .is_some_and(|t| agora.saturating_duration_since(*t) >= janela)
        {
            self.aceitos.pop_front();
        }
        if self.aceitos.len() >= EVENTOS_POR_SEGUNDO {
            return false;
        }
        self.aceitos.push_back(agora);
        true
    }
}

/// Converte um ponto do espaço da IMAGEM capturada (pixels físicos; é o que o cockpit manda) para o
/// espaço em que o sistema posiciona o mouse (`sistema`, pixels "lógicos"). Com a tela ampliada pelo
/// Windows (125%, 150%, 200%) os dois tamanhos diferem; sem a conversão o clique cai mais longe da
/// origem do que o ponto clicado (ex.: 150% → o clique em 960 cai em 1440). Devolve `None` se algum
/// tamanho for zero ou negativo (nada a calcular: melhor recusar que clicar no lugar errado).
pub fn escalar_para_o_sistema(
    (x, y): (i32, i32),
    captura: (u32, u32),
    sistema: (i32, i32),
) -> Option<(i32, i32)> {
    let (cw, ch) = (i64::from(captura.0), i64::from(captura.1));
    let (sw, sh) = (i64::from(sistema.0), i64::from(sistema.1));
    if cw <= 0 || ch <= 0 || sw <= 0 || sh <= 0 {
        return None;
    }
    // Regra de três com arredondamento ao mais próximo, e dentro da tela (0..=tamanho-1).
    let eixo = |v: i32, de: i64, para: i64| -> i32 {
        let v = i64::from(v).max(0);
        ((v * para * 2 + de) / (2 * de)).min(para - 1) as i32
    };
    Some((eixo(x, cw, sw), eixo(y, ch, sh)))
}

/// Injetor real (enigo). Só existe com `--features input-injection`.
#[cfg(feature = "input-injection")]
pub struct InjetorReal {
    enigo: std::sync::Mutex<enigo::Enigo>,
}

#[cfg(feature = "input-injection")]
impl InjetorReal {
    pub fn novo() -> Result<Self, String> {
        let enigo = enigo::Enigo::new(&enigo::Settings::default()).map_err(|e| e.to_string())?;
        Ok(Self {
            enigo: std::sync::Mutex::new(enigo),
        })
    }
}

#[cfg(feature = "input-injection")]
impl InjetorEntrada for InjetorReal {
    fn clique(&self, x: i32, y: i32, botao: &str) -> Result<(), String> {
        use enigo::{Button, Coordinate, Direction, Mouse};
        let b = match botao {
            "right" => Button::Right,
            "middle" => Button::Middle,
            _ => Button::Left,
        };
        let mut e = self
            .enigo
            .lock()
            .map_err(|_| "injetor travado".to_string())?;
        // O ponto vem em pixels da imagem capturada; o sistema posiciona o mouse no tamanho dele
        // (que difere com escala de DPI). Medido a cada clique: a resolução pode mudar.
        // ATALHO: enumera os monitores a cada clique (cliques humanos, no máximo 60/s); se pesar,
        // guardar o tamanho e refazer só quando o `main_display` mudar.
        let captura = zylos_computer_view::session::tamanho_do_monitor_primario()?;
        let sistema = e.main_display().map_err(|e| e.to_string())?;
        let (x, y) = escalar_para_o_sistema((x, y), captura, sistema)
            .ok_or_else(|| "não consegui medir a tela para posicionar o clique".to_string())?;
        e.move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| e.to_string())?;
        e.button(b, Direction::Click).map_err(|e| e.to_string())
    }

    fn tecla(&self, tecla: &str, apertada: bool) -> Result<(), String> {
        use enigo::{Direction, Key, Keyboard};
        let k = match tecla {
            "Enter" => Key::Return,
            "Backspace" => Key::Backspace,
            "Tab" => Key::Tab,
            "Escape" => Key::Escape,
            "ArrowUp" => Key::UpArrow,
            "ArrowDown" => Key::DownArrow,
            "ArrowLeft" => Key::LeftArrow,
            "ArrowRight" => Key::RightArrow,
            "Home" => Key::Home,
            "End" => Key::End,
            "PageUp" => Key::PageUp,
            "PageDown" => Key::PageDown,
            "Delete" => Key::Delete,
            "Shift" => Key::Shift,
            "Control" => Key::Control,
            "Alt" => Key::Alt,
            c => Key::Unicode(c.chars().next().ok_or("tecla vazia")?),
        };
        let dir = if apertada {
            Direction::Press
        } else {
            Direction::Release
        };
        let mut e = self
            .enigo
            .lock()
            .map_err(|_| "injetor travado".to_string())?;
        e.key(k, dir).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Anota o que receberia; nunca toca no mouse nem no teclado.
    #[derive(Default)]
    pub struct Falso(pub Mutex<Vec<String>>);

    impl InjetorEntrada for Falso {
        fn clique(&self, x: i32, y: i32, botao: &str) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(format!("clique {x},{y} {botao}"));
            Ok(())
        }
        fn tecla(&self, tecla: &str, apertada: bool) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(format!("tecla {tecla} {apertada}"));
            Ok(())
        }
    }

    fn sessao_com_controle() -> (Arc<ComputerSessionManager>, String, String) {
        let m = Arc::new(ComputerSessionManager::new(30));
        let ativa = m
            .create_session(zylos_computer_view::schema::CreateSessionRequest {
                bot_id: "b".into(),
                vnc_endpoint: None,
                metadata: None,
            })
            .id;
        m.update_state(&ativa, SessionState::Active).unwrap();
        let sem = m
            .create_session(zylos_computer_view::schema::CreateSessionRequest {
                bot_id: "b".into(),
                vnc_endpoint: None,
                metadata: None,
            })
            .id;
        (m, ativa, sem)
    }

    fn assumir(m: &ComputerSessionManager, id: &str) {
        m.take_control(
            id,
            zylos_computer_view::schema::TakeControlRequest {
                user_id: "u1".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn so_injeta_com_controle_humano_ativo_e_so_do_dono() {
        let (m, ativa, sem) = sessao_com_controle();
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m.clone());
        let u1 = Some("u1");

        // Sessão ativa mas sem ninguém no controle, e sessão ainda `connecting`: recusa.
        assert_eq!(
            e.clique(&ativa, u1, 10, 10, "left"),
            Err(Recusa::SessaoSemControle)
        );
        assert_eq!(e.tecla(&sem, u1, "a", true), Err(Recusa::SessaoSemControle));
        assert_eq!(
            e.clique("nao-existe", u1, 1, 1, "left"),
            Err(Recusa::SessaoSemControle)
        );
        assert!(falso.0.lock().unwrap().is_empty());

        // u1 assumiu o controle: só u1 passa; outro usuário e conexão sem login, não.
        assumir(&m, &ativa);
        assert_eq!(
            e.clique(&ativa, Some("u2"), 10, 20, "left"),
            Err(Recusa::OutroUsuario)
        );
        assert_eq!(
            e.clique(&ativa, None, 10, 20, "left"),
            Err(Recusa::OutroUsuario)
        );
        assert_eq!(
            e.tecla(&ativa, Some("u2"), "a", true),
            Err(Recusa::OutroUsuario)
        );
        assert!(
            falso.0.lock().unwrap().is_empty(),
            "outro usuário não injeta nada"
        );
        assert_eq!(e.clique(&ativa, u1, 10, 20, "left"), Ok(()));
        assert_eq!(e.tecla(&ativa, u1, "a", true), Ok(()));
        assert_eq!(
            *falso.0.lock().unwrap(),
            vec!["clique 10,20 left".to_string(), "tecla a true".to_string()]
        );

        // Ao liberar o controle, volta a recusar (até para o antigo dono).
        m.release_control(&ativa, "u1").unwrap();
        assert_eq!(
            e.clique(&ativa, u1, 1, 1, "left"),
            Err(Recusa::SessaoSemControle)
        );
    }

    #[test]
    fn sessao_vnc_nunca_recebe_entrada_no_computador_do_gateway() {
        let m = Arc::new(ComputerSessionManager::new(30));
        let id = m
            .create_session(zylos_computer_view::schema::CreateSessionRequest {
                bot_id: "b".into(),
                vnc_endpoint: Some("vnc://10.0.0.5:5900".into()),
                metadata: None,
            })
            .id;
        m.update_state(&id, SessionState::Active).unwrap();
        assumir(&m, &id);
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m);
        assert_eq!(
            e.clique(&id, Some("u1"), 100, 100, "left"),
            Err(Recusa::SessaoRemota)
        );
        assert_eq!(
            e.tecla(&id, Some("u1"), "a", true),
            Err(Recusa::SessaoRemota)
        );
        assert!(falso.0.lock().unwrap().is_empty());
    }

    #[test]
    fn soltar_funciona_depois_de_perder_o_controle() {
        let (m, ativa, _) = sessao_com_controle();
        assumir(&m, &ativa);
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m.clone());
        assert_eq!(e.tecla(&ativa, Some("u1"), "Shift", true), Ok(()));
        m.release_control(&ativa, "u1").unwrap();
        // Apertar agora é recusado; soltar não.
        assert!(e.tecla(&ativa, Some("u1"), "a", true).is_err());
        assert_eq!(e.soltar("Shift"), Ok(()));
        assert_eq!(e.soltar("Meta"), Err(Recusa::TeclaNaoPermitida));
        assert_eq!(
            *falso.0.lock().unwrap(),
            vec![
                "tecla Shift true".to_string(),
                "tecla Shift false".to_string()
            ]
        );
    }

    #[test]
    fn valida_botao_coordenada_e_tecla() {
        let (m, ativa, _) = sessao_com_controle();
        assumir(&m, &ativa);
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m);
        let u = Some("u1");
        assert_eq!(
            e.clique(&ativa, u, 1, 1, "back"),
            Err(Recusa::BotaoInvalido)
        );
        assert_eq!(
            e.clique(&ativa, u, -1, 1, "left"),
            Err(Recusa::CoordenadaInvalida)
        );
        assert_eq!(
            e.clique(&ativa, u, 1, COORD_MAX + 1, "left"),
            Err(Recusa::CoordenadaInvalida)
        );
        for ruim in ["Meta", "OS", "F4", "ContextMenu", "ab", "", "\u{7f}", "é"] {
            assert_eq!(
                e.tecla(&ativa, u, ruim, true),
                Err(Recusa::TeclaNaoPermitida),
                "{ruim:?}"
            );
        }
        for boa in [
            "a",
            "Z",
            "5",
            ".",
            "Enter",
            "Backspace",
            "ArrowLeft",
            "Control",
            " ",
        ] {
            assert_eq!(e.tecla(&ativa, u, boa, true), Ok(()), "{boa:?}");
        }
        assert!(falso.0.lock().unwrap().iter().all(|s| !s.contains("Meta")));
    }

    #[test]
    fn limite_de_taxa_corta_o_excesso_e_renova_a_janela() {
        let mut l = LimiteDeTaxa::new();
        let aceitos = (0..EVENTOS_POR_SEGUNDO + 40)
            .filter(|_| l.permite())
            .count();
        assert_eq!(aceitos, EVENTOS_POR_SEGUNDO);
        std::thread::sleep(Duration::from_millis(1050));
        assert!(l.permite(), "janela nova");
    }

    #[test]
    fn janela_deslizante_nunca_deixa_passar_o_dobro_na_virada() {
        let t0 = Instant::now();
        let mut l = LimiteDeTaxa::new();
        // 60 eventos no fim da primeira janela (t0 + 900 ms)...
        let fim = t0 + Duration::from_millis(900);
        assert_eq!(
            (0..EVENTOS_POR_SEGUNDO)
                .filter(|_| l.permite_em(fim))
                .count(),
            60
        );
        // ...e logo depois da virada da janela fixa (t0 + 1010 ms) nada novo cabe: só 110 ms depois.
        let virada = t0 + Duration::from_millis(1010);
        assert!(
            !l.permite_em(virada),
            "a janela fixa deixaria passar 60 aqui"
        );
        // Passado 1 s do primeiro lote, a vaga volta.
        assert!(l.permite_em(t0 + Duration::from_millis(1900)));
        // Recusado não ocupa vaga: depois do lote vencer, cabem 60 de novo.
        let mut l2 = LimiteDeTaxa::new();
        for _ in 0..500 {
            l2.permite_em(t0);
        }
        let depois = t0 + Duration::from_millis(1000);
        assert_eq!((0..100).filter(|_| l2.permite_em(depois)).count(), 60);
    }

    #[test]
    fn varias_threads_juntas_nao_passam_de_60_por_segundo() {
        let (m, ativa, _) = sessao_com_controle();
        assumir(&m, &ativa);
        let falso = Arc::new(Falso::default());
        let e = Arc::new(Entrada::new(falso.clone(), m));
        let aceitos = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut hs = Vec::new();
        for _ in 0..8 {
            let (e, ativa, aceitos) = (e.clone(), ativa.clone(), aceitos.clone());
            hs.push(std::thread::spawn(move || {
                for x in 0..50 {
                    if e.clique(&ativa, Some("u1"), x, 1, "left").is_ok() {
                        aceitos.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            }));
        }
        for h in hs {
            h.join().unwrap();
        }
        // 400 tentativas quase simultâneas: no máximo uma janela cheia (ou duas se a rodada cruzou 1 s).
        let n = aceitos.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            (EVENTOS_POR_SEGUNDO..=2 * EVENTOS_POR_SEGUNDO).contains(&n),
            "aceitos: {n}"
        );
        assert_eq!(
            falso.0.lock().unwrap().len(),
            n,
            "o injetor recebeu só o que foi aceito"
        );
    }

    #[test]
    fn trava_envenenada_nao_derruba_a_entrada() {
        let (m, ativa, _) = sessao_com_controle();
        assumir(&m, &ativa);
        let falso = Arc::new(Falso::default());
        let e = Arc::new(Entrada::new(falso.clone(), m));
        let e2 = e.clone();
        // Uma thread entra em pânico segurando a trava do estado.
        let _ = std::thread::spawn(move || {
            let _g = e2.estado();
            panic!("pânico de propósito");
        })
        .join();
        assert!(e.estado.is_poisoned());
        assert!(e.clique(&ativa, Some("u1"), 1, 1, "left").is_ok());
        assert!(e.tecla(&ativa, Some("u1"), "a", true).is_ok());
        assert!(e.soltar("a").is_ok());
    }

    #[test]
    fn clique_na_escala_do_windows_cai_onde_o_usuario_clicou() {
        // 100%: nada muda.
        let cem = ((1920, 1080), (1920, 1080));
        assert_eq!(
            escalar_para_o_sistema((960, 540), cem.0, cem.1),
            Some((960, 540))
        );
        // 150%: a captura tem 1920x1080 e o sistema posiciona o mouse em 1280x720.
        let cento_e_cinquenta = ((1920, 1080), (1280, 720));
        let esc = |p| escalar_para_o_sistema(p, cento_e_cinquenta.0, cento_e_cinquenta.1);
        assert_eq!(
            esc((960, 540)),
            Some((640, 360)),
            "o centro continua no centro"
        );
        assert_eq!(esc((0, 0)), Some((0, 0)));
        assert_eq!(
            esc((1919, 1079)),
            Some((1279, 719)),
            "o último pixel é o último pixel"
        );
        // 125% (1920x1080 → 1536x864) e 200% (3840x2160 → 1920x1080).
        assert_eq!(
            escalar_para_o_sistema((960, 540), (1920, 1080), (1536, 864)),
            Some((768, 432))
        );
        assert_eq!(
            escalar_para_o_sistema((1920, 1080), (3840, 2160), (1920, 1080)),
            Some((960, 540))
        );
        // Arredonda ao mais próximo (0,67 → 1; 1,33 → 1), não para baixo.
        assert_eq!(esc((1, 1)), Some((1, 1)));
        assert_eq!(esc((2, 2)), Some((1, 1)));
        // Fora da tela é puxado para dentro; tamanho zero ou negativo é recusado.
        assert_eq!(esc((5000, 5000)), Some((1279, 719)));
        assert_eq!(esc((-4, -4)), Some((0, 0)));
        assert_eq!(escalar_para_o_sistema((1, 1), (0, 1080), (1280, 720)), None);
        assert_eq!(escalar_para_o_sistema((1, 1), (1920, 1080), (0, 720)), None);
        assert_eq!(
            escalar_para_o_sistema((1, 1), (1920, 1080), (-1, 720)),
            None
        );
    }

    /// Mede a tela REAL desta máquina (não move o mouse nem clica): `cargo test -p zylos-gateway
    /// --features input-injection -- --ignored mede_a_tela`. Precisa de monitor.
    #[cfg(feature = "input-injection")]
    #[test]
    #[ignore = "precisa de um monitor"]
    fn mede_a_tela_real_e_o_centro_da_captura_cai_no_centro_do_sistema() {
        use enigo::Mouse;
        let captura = zylos_computer_view::session::tamanho_do_monitor_primario().unwrap();
        let e = enigo::Enigo::new(&enigo::Settings::default()).unwrap();
        let sistema = e.main_display().unwrap();
        eprintln!("captura {captura:?}, sistema (enigo) {sistema:?}");
        let (cx, cy) = (captura.0 as i32 / 2, captura.1 as i32 / 2);
        let (sx, sy) = escalar_para_o_sistema((cx, cy), captura, sistema).unwrap();
        eprintln!("centro da captura ({cx},{cy}) → ({sx},{sy}) no sistema");
        assert!((sx - sistema.0 / 2).abs() <= 1 && (sy - sistema.1 / 2).abs() <= 1);
    }

    #[test]
    fn o_dono_da_concessao_inclui_o_tenant_e_so_admin_injeta() {
        use zylos_auth::Claims;
        let t1 = Claims::new("alice".into(), "t1".into(), vec!["admin".into()], 600);
        let t2 = Claims::new("alice".into(), "t2".into(), vec!["admin".into()], 600);
        assert_eq!(dono_da_concessao(&t1), "t1:alice");
        assert_ne!(dono_da_concessao(&t1), dono_da_concessao(&t2));
        assert!(pode_injetar(Some(&t1)));
        let leitor = Claims::new("alice".into(), "t1".into(), vec!["viewer".into()], 600);
        assert!(!pode_injetar(Some(&leitor)));
        let sem_papel = Claims::new("alice".into(), "t1".into(), vec![], 600);
        assert!(!pode_injetar(Some(&sem_papel)));
        // Sem login não há o que conferir aqui (a Entrada recusa quem não tem usuário).
        assert!(pode_injetar(None));
    }

    #[test]
    fn a_tecla_de_espaco_e_aceita_pelo_nome() {
        assert_eq!(TECLA_ESPACO, " ");
        assert!(tecla_permitida(TECLA_ESPACO));
    }

    #[test]
    fn so_o_valor_1_liga_pela_variavel() {
        assert!(!ligada_valor(None));
        assert!(ligada_valor(Some("1")));
        assert!(ligada_valor(Some(" 1 ")));
        for v in ["", "0", "true", "11", "yes"] {
            assert!(!ligada_valor(Some(v)), "{v:?}");
        }
    }

    /// Duas sessões ativas, cada uma com um dono diferente (`u1` e `u2`).
    fn duas_sessoes() -> (Arc<ComputerSessionManager>, String, String) {
        let (m, a, b) = sessao_com_controle();
        m.update_state(&b, SessionState::Active).unwrap();
        assumir(&m, &a);
        m.take_control(
            &b,
            zylos_computer_view::schema::TakeControlRequest {
                user_id: "u2".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
        (m, a, b)
    }

    #[test]
    fn atalhos_do_sistema_sao_recusados_em_qualquer_ordem() {
        let (m, ativa, _) = sessao_com_controle();
        assumir(&m, &ativa);
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m);
        let u = Some("u1");
        let p = |t: &str| e.tecla(&ativa, u, t, true);
        let s = |t: &str| e.tecla(&ativa, u, t, false);

        // Ctrl+Esc, Ctrl+Shift+Esc, Alt+Esc, Alt+Tab.
        assert_eq!(p("Control"), Ok(()));
        assert_eq!(p("Escape"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(p("Shift"), Ok(()));
        assert_eq!(p("Escape"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(s("Shift"), Ok(()));
        assert_eq!(s("Control"), Ok(()));
        assert_eq!(p("Alt"), Ok(()));
        assert_eq!(p("Tab"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(p("Escape"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(s("Alt"), Ok(()));
        // Ordem inversa: Esc/Tab já apertadas e só depois o modificador.
        assert_eq!(p("Escape"), Ok(()));
        assert_eq!(p("Control"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(p("Alt"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(s("Escape"), Ok(()));
        assert_eq!(p("Tab"), Ok(()));
        assert_eq!(p("Alt"), Err(Recusa::AtalhoBloqueado));
        assert_eq!(s("Tab"), Ok(()));
        // Sem modificador (ou depois de soltá-lo), Esc e Tab continuam funcionando; Ctrl+Tab não é bloqueado.
        assert_eq!(p("Escape"), Ok(()));
        assert_eq!(s("Escape"), Ok(()));
        assert_eq!(p("Control"), Ok(()));
        assert_eq!(p("Tab"), Ok(()));

        // Só chegou ao injetor o que NÃO armava um atalho; cada recusa acima ficou de fora.
        let esperado: Vec<String> = [
            "Control true",
            "Shift true",
            "Shift false",
            "Control false",
            "Alt true",
            "Alt false",
            "Escape true",
            "Escape false",
            "Tab true",
            "Tab false",
            "Escape true",
            "Escape false",
            "Control true",
            "Tab true",
        ]
        .iter()
        .map(|x| format!("tecla {x}"))
        .collect();
        assert_eq!(*falso.0.lock().unwrap(), esperado);
    }

    #[test]
    fn so_uma_sessao_por_vez_segura_a_maquina_e_a_vez_passa_quando_o_dono_larga() {
        let (m, a, b) = duas_sessoes();
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m.clone());
        assert_eq!(e.clique(&a, Some("u1"), 1, 1, "left"), Ok(()));
        // u2 tem controle válido na outra sessão, mas a máquina está com u1.
        assert_eq!(
            e.clique(&b, Some("u2"), 2, 2, "left"),
            Err(Recusa::MaquinaOcupada)
        );
        assert_eq!(
            e.tecla(&b, Some("u2"), "x", true),
            Err(Recusa::MaquinaOcupada)
        );
        assert_eq!(
            *falso.0.lock().unwrap(),
            vec!["clique 1,1 left".to_string()]
        );
        // u1 larga o controle: a vez passa para u2.
        m.release_control(&a, "u1").unwrap();
        assert_eq!(e.clique(&b, Some("u2"), 3, 3, "left"), Ok(()));
        assert_eq!(
            *falso.0.lock().unwrap(),
            vec!["clique 1,1 left".to_string(), "clique 3,3 left".to_string()]
        );
    }

    #[test]
    fn dono_ocioso_perde_a_vez() {
        let (m, a, b) = duas_sessoes();
        let e = Entrada::new(Arc::new(Falso::default()), m);
        assert_eq!(e.clique(&a, Some("u1"), 1, 1, "left"), Ok(()));
        // Faz de conta que o último evento foi há mais que POSSE_OCIOSA.
        e.estado.lock().unwrap().ultimo_evento = Some(Instant::now() - POSSE_OCIOSA * 2);
        assert_eq!(e.clique(&b, Some("u2"), 2, 2, "left"), Ok(()));
    }

    #[test]
    fn troca_de_dono_solta_antes_as_teclas_que_o_anterior_deixou_apertadas() {
        let (m, a, _) = sessao_com_controle();
        assumir(&m, &a);
        let falso = Arc::new(Falso::default());
        let e = Entrada::new(falso.clone(), m.clone());
        assert_eq!(e.tecla(&a, Some("u1"), "Control", true), Ok(()));
        // u1 perde o controle e u2 assume a MESMA sessão.
        m.release_control(&a, "u1").unwrap();
        m.take_control(
            &a,
            zylos_computer_view::schema::TakeControlRequest {
                user_id: "u2".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
        assert_eq!(e.tecla(&a, Some("u2"), "c", true), Ok(()));
        assert_eq!(
            *falso.0.lock().unwrap(),
            vec![
                "tecla Control true".to_string(),
                "tecla Control false".to_string(),
                "tecla c true".to_string()
            ],
            "o Control do u1 é solto antes do primeiro evento do u2 (senão virava Ctrl+C)"
        );
    }

    #[test]
    fn limite_de_60_por_segundo_vale_para_a_maquina_toda() {
        let (m, ativa, _) = sessao_com_controle();
        assumir(&m, &ativa);
        let e = Entrada::new(Arc::new(Falso::default()), m);
        let aceitos = (0..EVENTOS_POR_SEGUNDO + 40)
            .filter(|i| e.clique(&ativa, Some("u1"), *i as i32, 1, "left").is_ok())
            .count();
        assert_eq!(aceitos, EVENTOS_POR_SEGUNDO);
        assert_eq!(
            e.clique(&ativa, Some("u1"), 1, 1, "left"),
            Err(Recusa::EventosDemais)
        );
        // Soltar uma tecla segue passando mesmo estourado.
        assert_eq!(e.soltar("Shift"), Ok(()));
    }

    #[test]
    fn a_entrada_so_liga_com_as_tres_travas() {
        for pedida in [false, true] {
            for captura in [false, true] {
                for injetor in [false, true] {
                    let esperado = match (pedida, captura, injetor) {
                        (false, _, _) => Porta::Desligada,
                        (true, false, _) => Porta::SemCaptura,
                        (true, true, false) => Porta::SemInjetor,
                        (true, true, true) => Porta::Ligar,
                    };
                    assert_eq!(decidir_porta(pedida, captura, injetor), esperado);
                }
            }
        }
        // Só UMA combinação liga.
        let ligam = [false, true]
            .iter()
            .flat_map(|p| [false, true].iter().map(move |c| (*p, *c)))
            .flat_map(|(p, c)| [false, true].iter().map(move |i| (p, c, *i)))
            .filter(|(p, c, i)| decidir_porta(*p, *c, *i) == Porta::Ligar)
            .count();
        assert_eq!(ligam, 1);
    }
}
