use std::time::Instant;

use slt::{Border, Color, Context, KeyCode};

use crate::games::{self, GameSignal, RunningGame};
use crate::records::RecordsStore;

pub struct App {
    records: RecordsStore,
    screen: Screen,
    selected_game: usize,
    last_frame: Instant,
}

enum Screen {
    Menu,
    Game(RunningGame),
}

impl App {
    pub fn new() -> Self {
        Self {
            records: RecordsStore::load(),
            screen: Screen::Menu,
            selected_game: 0,
            last_frame: Instant::now(),
        }
    }

    pub fn frame(&mut self, ui: &mut Context) {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_frame);
        self.last_frame = now;

        if ui.key('q') {
            ui.quit();
            return;
        }

        match &mut self.screen {
            Screen::Menu => {
                self.handle_menu_input(ui);
                self.render_menu(ui);
            }
            Screen::Game(game) => {
                if ui.key('z') {
                    game.sync_records(&mut self.records);
                    self.screen = Screen::Menu;
                    return;
                }

                if game.frame(ui, delta) == GameSignal::ReturnToMenu {
                    game.sync_records(&mut self.records);
                    self.screen = Screen::Menu;
                } else {
                    game.sync_records(&mut self.records);
                }
            }
        }
    }

    fn handle_menu_input(&mut self, ui: &mut Context) {
        let game_count = games::catalog().len();
        if game_count == 0 {
            return;
        }

        if ui.key('k') || ui.key_code(KeyCode::Up) {
            self.selected_game = if self.selected_game == 0 {
                game_count - 1
            } else {
                self.selected_game - 1
            };
        }

        if ui.key('j') || ui.key_code(KeyCode::Down) {
            self.selected_game = (self.selected_game + 1) % game_count;
        }

        if ui.key_code(KeyCode::Enter) || ui.key(' ') {
            let selected = games::catalog()[self.selected_game].id;
            self.screen = Screen::Game(RunningGame::new(selected, self.records.records()));
            self.last_frame = Instant::now();
        }
    }

    fn render_menu(&self, ui: &mut Context) {
        let games = games::catalog();

        if ui.width() < 52 || ui.height() < 20 {
            let _ = ui
                .bordered(Border::Rounded)
                .title("Arcade")
                .p(1)
                .gap(1)
                .col(|ui| {
                    ui.text("Terminal too small. Resize to at least 52x20.")
                        .fg(Color::Yellow);
                    ui.text("Enter start");
                    ui.text("q quit");
                });
            return;
        }

        let _ = ui
            .bordered(Border::Rounded)
            .title("Arcade")
            .p(1)
            .gap(1)
            .col(|ui| {
                ui.text("Game Select").bold().fg(Color::LightCyan);
                ui.text("Choose a game and press Enter.").dim();

                let _ = ui.container().gap(2).row(|ui| {
                    let _ = ui
                        .bordered(Border::Rounded)
                        .title("Games")
                        .w(28)
                        .p(1)
                        .gap(1)
                        .col(|ui| {
                            for (index, game) in games.iter().enumerate() {
                                let is_selected = index == self.selected_game;
                                let prefix = if is_selected { ">" } else { " " };
                                let color = if is_selected {
                                    Color::LightGreen
                                } else {
                                    Color::White
                                };

                                ui.text(format!("{prefix} {}", game.name)).bold().fg(color);
                            }
                        });

                    let selected = &games[self.selected_game];
                    let _ = ui
                        .bordered(Border::Rounded)
                        .title("Details")
                        .w(28)
                        .p(1)
                        .gap(1)
                        .col(|ui| {
                            ui.text(selected.name).bold().fg(Color::LightYellow);
                            ui.text(selected.description);
                            ui.separator();
                            ui.text("More games can be added under src/games/.").dim();
                        });
                });

                let _ = ui.help(&[
                    ("j/k", "move"),
                    ("↑/↓", "move"),
                    ("Enter", "start"),
                    ("q", "quit"),
                ]);
            });
    }
}
