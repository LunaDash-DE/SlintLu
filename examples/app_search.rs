//! Searches installed applications: `cargo run --example app_search -- term`.

use slintlu::launcher::{LaunchHistory, application_dirs, scan, search};

fn main() {
    let query = std::env::args().nth(1).unwrap_or_default();
    let language = slintlu::i18n::current_language();
    let start = std::time::Instant::now();
    let apps = scan(&application_dirs(), language.as_deref());
    println!("{} applications in {:?}", apps.len(), start.elapsed());
    let history = LaunchHistory::load(LaunchHistory::default_path());
    for app in search(&apps, &query, &history, 8) {
        println!("{:<40} {:<30} {}", app.name, app.id, app.exec);
    }
}
