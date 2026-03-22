use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use slt::{Border, Breakpoint, Color, Context, KeyCode, RunConfig};

const BOARD_WIDTH: usize = 10;
const BOARD_HEIGHT: usize = 20;
const PREVIEW_SIZE: usize = 4;
const SIMPLE_KICKS: [i32; 5] = [0, -1, 1, -2, 2];
const EMPTY_CELL: Color = Color::Indexed(236);

type Board = [[Option<TetrominoKind>; BOARD_WIDTH]; BOARD_HEIGHT];

fn main() -> std::io::Result<()> {
    let mut app = App::new();

    slt::run_with(
        RunConfig::default().title("Tetris"),
        |ui: &mut Context| app.frame(ui),
    )
}

struct App {
    game: Game,
    phase: Phase,
    last_frame: Instant,
    gravity_accumulator: Duration,
}

impl App {
    fn new() -> Self {
        Self {
            game: Game::new(),
            phase: Phase::Playing,
            last_frame: Instant::now(),
            gravity_accumulator: Duration::ZERO,
        }
    }

    fn restart(&mut self) {
        self.game = Game::new();
        self.phase = Phase::Playing;
        self.gravity_accumulator = Duration::ZERO;
        self.last_frame = Instant::now();
    }

    fn frame(&mut self, ui: &mut Context) {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_frame);
        self.last_frame = now;

        if ui.key('q') {
            ui.quit();
            return;
        }

        if ui.key('r') {
            self.restart();
        }

        if ui.key('p') {
            self.phase = match self.phase {
                Phase::Playing => {
                    self.gravity_accumulator = Duration::ZERO;
                    Phase::Paused
                }
                Phase::Paused => {
                    self.gravity_accumulator = Duration::ZERO;
                    Phase::Playing
                }
                Phase::GameOver => Phase::GameOver,
            };
        }

        if self.phase == Phase::Playing {
            self.handle_input(ui);
            self.advance_gravity(delta);
        }

        render_app(ui, self);
    }

    fn handle_input(&mut self, ui: &mut Context) {
        if ui.key('h') || ui.key_code(KeyCode::Left) {
            self.game.try_move(-1, 0);
        }
        if ui.key('l') || ui.key_code(KeyCode::Right) {
            self.game.try_move(1, 0);
        }
        if ui.key('k') || ui.key('x') || ui.key_code(KeyCode::Up) {
            self.game.try_rotate();
        }
        if ui.key('j') || ui.key_code(KeyCode::Down) {
            if !self.game.soft_drop() {
                self.phase = Phase::GameOver;
            }
        }
        if ui.key(' ') {
            if !self.game.hard_drop() {
                self.phase = Phase::GameOver;
            }
            self.gravity_accumulator = Duration::ZERO;
        }
    }

    fn advance_gravity(&mut self, delta: Duration) {
        self.gravity_accumulator += delta;
        let interval = self.game.gravity_interval();

        while self.gravity_accumulator >= interval {
            self.gravity_accumulator -= interval;
            if !self.game.advance_gravity() {
                self.phase = Phase::GameOver;
                self.gravity_accumulator = Duration::ZERO;
                break;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Playing,
    Paused,
    GameOver,
}

struct Game {
    board: Board,
    current: ActivePiece,
    next: TetrominoKind,
    randomizer: BagRandomizer,
    score: u32,
    lines: u32,
    level: u32,
}

impl Game {
    fn new() -> Self {
        let mut randomizer = BagRandomizer::new();
        let current_kind = randomizer.next();
        let next = randomizer.next();

        Self {
            board: empty_board(),
            current: ActivePiece::spawn(current_kind),
            next,
            randomizer,
            score: 0,
            lines: 0,
            level: 1,
        }
    }

    fn gravity_interval(&self) -> Duration {
        let millis = match self.level {
            1 => 700,
            2 => 620,
            3 => 540,
            4 => 470,
            5 => 400,
            6 => 340,
            7 => 290,
            8 => 240,
            9 => 200,
            10 => 170,
            _ => 140,
        };

        Duration::from_millis(millis)
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let candidate = self.current.shifted(dx, dy);
        if self.collides(candidate) {
            return false;
        }
        self.current = candidate;
        true
    }

    fn try_rotate(&mut self) -> bool {
        let rotated = self.current.rotated();
        for kick in SIMPLE_KICKS {
            let candidate = rotated.shifted(kick, 0);
            if !self.collides(candidate) {
                self.current = candidate;
                return true;
            }
        }
        false
    }

    fn soft_drop(&mut self) -> bool {
        if self.try_move(0, 1) {
            self.score = self.score.saturating_add(1);
            true
        } else {
            self.lock_and_spawn()
        }
    }

    fn hard_drop(&mut self) -> bool {
        let mut dropped_rows = 0_u32;
        while self.try_move(0, 1) {
            dropped_rows = dropped_rows.saturating_add(1);
        }
        self.score = self.score.saturating_add(dropped_rows.saturating_mul(2));
        self.lock_and_spawn()
    }

    fn advance_gravity(&mut self) -> bool {
        if self.try_move(0, 1) {
            true
        } else {
            self.lock_and_spawn()
        }
    }

    fn lock_and_spawn(&mut self) -> bool {
        for (x, y) in self.current.cells() {
            if y >= 0 && y < BOARD_HEIGHT as i32 && x >= 0 && x < BOARD_WIDTH as i32 {
                self.board[y as usize][x as usize] = Some(self.current.kind);
            }
        }

        let cleared = self.clear_lines();
        self.apply_score(cleared);

        self.current = ActivePiece::spawn(self.next);
        self.next = self.randomizer.next();

        !self.collides(self.current)
    }

    fn clear_lines(&mut self) -> u32 {
        let mut next_board = empty_board();
        let mut write_y = BOARD_HEIGHT;
        let mut cleared = 0_u32;

        for y in (0..BOARD_HEIGHT).rev() {
            if self.board[y].iter().all(Option::is_some) {
                cleared = cleared.saturating_add(1);
            } else {
                write_y -= 1;
                next_board[write_y] = self.board[y];
            }
        }

        self.board = next_board;
        cleared
    }

    fn apply_score(&mut self, cleared: u32) {
        let line_score: u32 = match cleared {
            1 => 100,
            2 => 300,
            3 => 500,
            4 => 800,
            _ => 0,
        };

        self.score = self
            .score
            .saturating_add(line_score.saturating_mul(self.level));
        self.lines = self.lines.saturating_add(cleared);
        self.level = self.lines / 10 + 1;
    }

    fn collides(&self, piece: ActivePiece) -> bool {
        piece.cells().into_iter().any(|(x, y)| {
            x < 0
                || x >= BOARD_WIDTH as i32
                || y < 0
                || y >= BOARD_HEIGHT as i32
                || self.board[y as usize][x as usize].is_some()
        })
    }

    fn cell_at(&self, x: usize, y: usize) -> Option<TetrominoKind> {
        for (cell_x, cell_y) in self.current.cells() {
            if cell_x == x as i32 && cell_y == y as i32 {
                return Some(self.current.kind);
            }
        }
        self.board[y][x]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActivePiece {
    kind: TetrominoKind,
    rotation: usize,
    x: i32,
    y: i32,
}

impl ActivePiece {
    fn spawn(kind: TetrominoKind) -> Self {
        Self {
            kind,
            rotation: 0,
            x: 3,
            y: 0,
        }
    }

    fn shifted(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }

    fn rotated(self) -> Self {
        Self {
            rotation: (self.rotation + 1) % 4,
            ..self
        }
    }

    fn cells(self) -> [(i32, i32); 4] {
        self.kind
            .offsets(self.rotation)
            .map(|(dx, dy)| (self.x + dx, self.y + dy))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TetrominoKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl TetrominoKind {
    fn color(self) -> Color {
        match self {
            TetrominoKind::I => Color::Cyan,
            TetrominoKind::O => Color::Yellow,
            TetrominoKind::T => Color::Magenta,
            TetrominoKind::S => Color::Green,
            TetrominoKind::Z => Color::Red,
            TetrominoKind::J => Color::Blue,
            TetrominoKind::L => Color::LightYellow,
        }
    }

    fn offsets(self, rotation: usize) -> [(i32, i32); 4] {
        match self {
            TetrominoKind::I => match rotation % 4 {
                0 => [(0, 1), (1, 1), (2, 1), (3, 1)],
                1 => [(2, 0), (2, 1), (2, 2), (2, 3)],
                2 => [(0, 2), (1, 2), (2, 2), (3, 2)],
                _ => [(1, 0), (1, 1), (1, 2), (1, 3)],
            },
            TetrominoKind::O => [(1, 0), (2, 0), (1, 1), (2, 1)],
            TetrominoKind::T => match rotation % 4 {
                0 => [(1, 0), (0, 1), (1, 1), (2, 1)],
                1 => [(1, 0), (1, 1), (2, 1), (1, 2)],
                2 => [(0, 1), (1, 1), (2, 1), (1, 2)],
                _ => [(1, 0), (0, 1), (1, 1), (1, 2)],
            },
            TetrominoKind::S => match rotation % 4 {
                0 => [(1, 0), (2, 0), (0, 1), (1, 1)],
                1 => [(1, 0), (1, 1), (2, 1), (2, 2)],
                2 => [(1, 1), (2, 1), (0, 2), (1, 2)],
                _ => [(0, 0), (0, 1), (1, 1), (1, 2)],
            },
            TetrominoKind::Z => match rotation % 4 {
                0 => [(0, 0), (1, 0), (1, 1), (2, 1)],
                1 => [(2, 0), (1, 1), (2, 1), (1, 2)],
                2 => [(0, 1), (1, 1), (1, 2), (2, 2)],
                _ => [(1, 0), (0, 1), (1, 1), (0, 2)],
            },
            TetrominoKind::J => match rotation % 4 {
                0 => [(0, 0), (0, 1), (1, 1), (2, 1)],
                1 => [(1, 0), (2, 0), (1, 1), (1, 2)],
                2 => [(0, 1), (1, 1), (2, 1), (2, 2)],
                _ => [(1, 0), (1, 1), (0, 2), (1, 2)],
            },
            TetrominoKind::L => match rotation % 4 {
                0 => [(2, 0), (0, 1), (1, 1), (2, 1)],
                1 => [(1, 0), (1, 1), (1, 2), (2, 2)],
                2 => [(0, 1), (1, 1), (2, 1), (0, 2)],
                _ => [(0, 0), (1, 0), (1, 1), (1, 2)],
            },
        }
    }
}

struct BagRandomizer {
    state: u64,
    bag: [TetrominoKind; 7],
    index: usize,
}

impl BagRandomizer {
    fn new() -> Self {
        let mut randomizer = Self {
            state: seed(),
            bag: all_pieces(),
            index: 7,
        };
        randomizer.refill();
        randomizer
    }

    fn next(&mut self) -> TetrominoKind {
        if self.index >= self.bag.len() {
            self.refill();
        }

        let piece = self.bag[self.index];
        self.index += 1;
        piece
    }

    fn refill(&mut self) {
        self.bag = all_pieces();
        for i in (1..self.bag.len()).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            self.bag.swap(i, j);
        }
        self.index = 0;
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x as u32
    }
}

fn render_app(ui: &mut Context, app: &App) {
    if ui.width() < 52 || ui.height() < 28 {
        let _ = ui
            .bordered(Border::Rounded)
            .title("Tetris")
            .p(1)
            .gap(1)
            .col(|ui| {
                ui.text("Terminal too small. Resize to at least 52x28.").fg(Color::Yellow);
                ui.text("r restart");
                ui.text("q quit");
            });
        return;
    }

    let _ = ui
        .bordered(Border::Rounded)
        .title("Tetris")
        .p(1)
        .gap(1)
        .col(|ui| {
            ui.text("superlighttui tetris").bold().fg(Color::LightCyan);

            if ui.breakpoint() == Breakpoint::Xs || ui.breakpoint() == Breakpoint::Sm {
                let _ = ui.container().gap(1).col(|ui| {
                    render_board(ui, &app.game);
                    render_sidebar(ui, app);
                });
            } else {
                let _ = ui.container().gap(2).row(|ui| {
                    render_board(ui, &app.game);
                    render_sidebar(ui, app);
                });
            }
        });
}

fn render_board(ui: &mut Context, game: &Game) {
    let _ = ui
        .bordered(Border::Rounded)
        .title("Board")
        .p(1)
        .col(|ui| {
            let _ = ui.container().gap(0).col(|ui| {
                for y in 0..BOARD_HEIGHT {
                    let _ = ui.container().gap(0).row(|ui| {
                        for x in 0..BOARD_WIDTH {
                            draw_cell(ui, game.cell_at(x, y));
                        }
                    });
                }
            });
        });
}

fn render_sidebar(ui: &mut Context, app: &App) {
    let _ = ui.container().w(24).gap(1).col(|ui| {
        let _ = ui
            .bordered(Border::Rounded)
            .title("Status")
            .p(1)
            .gap(1)
            .col(|ui| {
                let (label, color) = match app.phase {
                    Phase::Playing => ("Playing", Color::LightGreen),
                    Phase::Paused => ("Paused", Color::LightYellow),
                    Phase::GameOver => ("Game Over", Color::LightRed),
                };

                ui.text(label).bold().fg(color);
                if app.phase == Phase::GameOver {
                    ui.text("Press r to start again.").dim();
                }
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Stats")
            .p(1)
            .gap(1)
            .col(|ui| {
                let _ = ui.stat("Score", &app.game.score.to_string());
                let _ = ui.stat("Lines", &app.game.lines.to_string());
                let _ = ui.stat("Level", &app.game.level.to_string());
                let _ = ui.stat(
                    "Speed",
                    &format!("{} ms", app.game.gravity_interval().as_millis()),
                );
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Next")
            .p(1)
            .col(|ui| {
                render_next_piece(ui, app.game.next);
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Controls")
            .p(1)
            .gap(0)
            .col(|ui| {
                ui.text("Left/Right : move").dim();
                ui.text("Down       : soft drop").dim();
                ui.text("Up or x    : rotate").dim();
                ui.text("Space      : hard drop").dim();
                ui.text("p          : pause").dim();
                ui.text("r          : restart").dim();
                ui.text("q          : quit").dim();
            });
    });
}

fn render_next_piece(ui: &mut Context, kind: TetrominoKind) {
    let piece = ActivePiece {
        kind,
        rotation: 0,
        x: 0,
        y: 0,
    };

    let _ = ui.container().gap(0).col(|ui| {
        for y in 0..PREVIEW_SIZE {
            let _ = ui.container().gap(0).row(|ui| {
                for x in 0..PREVIEW_SIZE {
                    let filled = piece
                        .cells()
                        .into_iter()
                        .any(|(cell_x, cell_y)| cell_x == x as i32 && cell_y == y as i32);
                    draw_cell(ui, filled.then_some(kind));
                }
            });
        }
    });
}

fn draw_cell(ui: &mut Context, cell: Option<TetrominoKind>) {
    let color = cell.map_or(EMPTY_CELL, TetrominoKind::color);
    ui.text("  ").bg(color);
}

fn empty_board() -> Board {
    [[None; BOARD_WIDTH]; BOARD_HEIGHT]
}

fn all_pieces() -> [TetrominoKind; 7] {
    [
        TetrominoKind::I,
        TetrominoKind::O,
        TetrominoKind::T,
        TetrominoKind::S,
        TetrominoKind::Z,
        TetrominoKind::J,
        TetrominoKind::L,
    ]
}

fn seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);

    nanos ^ 0x9E37_79B9_7F4A_7C15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_clear_removes_full_rows() {
        let mut game = Game::new();
        game.board[BOARD_HEIGHT - 1] = [Some(TetrominoKind::I); BOARD_WIDTH];
        game.board[BOARD_HEIGHT - 2][0] = Some(TetrominoKind::O);

        let cleared = game.clear_lines();

        assert_eq!(cleared, 1);
        assert_eq!(game.board[BOARD_HEIGHT - 1][0], Some(TetrominoKind::O));
        assert!(game.board[0].iter().all(Option::is_none));
    }

    #[test]
    fn collision_detects_walls() {
        let game = Game::new();
        let piece = ActivePiece {
            kind: TetrominoKind::I,
            rotation: 0,
            x: -1,
            y: 0,
        };

        assert!(game.collides(piece));
    }

    #[test]
    fn rotate_uses_simple_wall_kick() {
        let mut game = Game::new();
        game.current = ActivePiece {
            kind: TetrominoKind::L,
            rotation: 0,
            x: BOARD_WIDTH as i32 - 3,
            y: 0,
        };

        assert!(game.try_rotate());
        assert!(game.current.x <= BOARD_WIDTH as i32 - 3);
    }

    #[test]
    fn spawn_failure_returns_game_over() {
        let mut game = Game::new();
        for y in 0..4 {
            for x in 3..7 {
                game.board[y][x] = Some(TetrominoKind::T);
            }
        }
        game.current = ActivePiece {
            kind: TetrominoKind::O,
            rotation: 0,
            x: 3,
            y: 18,
        };

        assert!(!game.lock_and_spawn());
    }

    #[test]
    fn scoring_updates_with_tetris_clear() {
        let mut game = Game::new();
        game.level = 3;
        game.apply_score(4);

        assert_eq!(game.score, 2400);
        assert_eq!(game.lines, 4);
        assert_eq!(game.level, 1);
    }
}
