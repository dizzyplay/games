mod app;
mod games;

use slt::{Context, RunConfig};

fn main() -> std::io::Result<()> {
    let mut app = app::App::new();

    slt::run_with(
        RunConfig::default().title("Arcade"),
        |ui: &mut Context| app.frame(ui),
    )
}
