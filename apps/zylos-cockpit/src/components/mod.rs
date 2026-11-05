//! Zylos Cockpit — Components Module

pub mod agent_card;
pub mod lazy_roster;
pub mod soar_dashboard;
pub mod theme_toggle;

#[allow(unused_imports)]
pub use agent_card::{AgentCard, AgentInfo, AgentStatus};
pub use lazy_roster::LazyRoster;
pub use soar_dashboard::SoarDashboard;
pub use theme_toggle::{ThemeToggle, provide_theme};
#[allow(unused_imports)]
pub use theme_toggle::{ThemeContext, Theme, use_theme};