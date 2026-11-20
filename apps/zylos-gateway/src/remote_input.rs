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
//! 4. **Coordenadas limitadas** (0 a 16384) e **taxa limitada** (60 eventos por segundo por conexão).
//!
//! O injetor é um trait: os testes usam um falso que só anota o que receberia, então nenhum teste
//! mexe no mouse ou no teclado de verdade.

use std::sync::Arc;
use std::time::{Duration, Instant};

use zylos_computer_view::schema::SessionState;
use zylos_computer_view::session::ComputerSessionManager;

/// Variável que liga a entrada remota (padrão: desligada).
pub const ENTRADA_ENV: &str = "ZYLOS_COMPUTER_VIEW_INPUT";
/// Maior coordenada aceita (px do quadro).
pub const COORD_MAX: i32 = 16_384;
/// Eventos de entrada por segundo, por conexão.
pub const EVENTOS_POR_SEGUNDO: usize = 60;

pub fn ligada_por_env() -> bool {
    std::env::var(ENTRADA_ENV).is_ok_and(|v| v.trim() == "1")
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
            Self::Falha(e) => format!("Falha ao enviar a entrada: {e}"),
        }
    }
}

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
        " ",
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
}

impl Entrada {
    pub fn new(injetor: Arc<dyn InjetorEntrada>, manager: Arc<ComputerSessionManager>) -> Self {
        Self { injetor, manager }
    }

    /// A sessão tem controle humano ativo, com a concessão ainda válida, e `usuario` (o `sub` do
    /// JWT da conexão, `None` se não autenticada) é o dono dela?
    pub fn autorizar(&self, session_id: &str, usuario: Option<&str>) -> Result<(), Recusa> {
        let sessao = self
            .manager
            .get_session(session_id)
            .filter(|s| s.state == SessionState::UserControl.as_str())
            .ok_or(Recusa::SessaoSemControle)?;
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
        self.injetor.tecla(tecla, apertada).map_err(Recusa::Falha)
    }
}

/// Limite de eventos por segundo de uma conexão (janela fixa de 1 s).
pub struct LimiteDeTaxa {
    inicio: Instant,
    contados: usize,
}

impl Default for LimiteDeTaxa {
    fn default() -> Self {
        Self::new()
    }
}

impl LimiteDeTaxa {
    pub fn new() -> Self {
        Self {
            inicio: Instant::now(),
            contados: 0,
        }
    }

    /// `true` se este evento cabe na janela atual.
    pub fn permite(&mut self) -> bool {
        if self.inicio.elapsed() >= Duration::from_secs(1) {
            self.inicio = Instant::now();
            self.contados = 0;
        }
        self.contados += 1;
        self.contados <= EVENTOS_POR_SEGUNDO
    }
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
    fn desligada_por_padrao() {
        if std::env::var(ENTRADA_ENV).is_err() {
            assert!(!ligada_por_env());
        }
    }
}
