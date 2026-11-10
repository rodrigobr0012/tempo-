//! Páginas do app.

mod history;
mod onboarding;
mod plan;
mod settings;
mod today;

pub use history::HistoryPage;
pub use onboarding::Onboarding;
pub use plan::{PlanPage, ReservedList};
pub use settings::SettingsPage;
pub use today::{toggle_hero, TodayPage};
