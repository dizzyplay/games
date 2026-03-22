use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slt::{Border, Breakpoint, Color, Context, KeyCode};

use super::GameSignal;

const BOARD_WIDTH: usize = 10;
const BOARD_HEIGHT: usize = 20;
const PREVIEW_SIZE: usize = 4;
const SIMPLE_KICKS: [i32; 5] = [0, -1, 1, -2, 2];
const EMPTY_CELL: Color = Color::Indexed(236);
const GHOST_CELL: Color = Color::Indexed(244);

type Board = [[Option<TetrominoKind>; BOARD_WIDTH]; BOARD_HEIGHT];

pub struct TetrisGame {
    game: Game,
    phase: Phase,
    gravity_accumulator: Duration,
}

impl TetrisGame {
    pub fn new() -> Self {
        Self {
            game: Game::new(),
            phase: Phase::Playing,
            gravity_accumulator: Duration::ZERO,
        }
    }

    pub fn frame(&mut self, ui: &mut Context, delta: Duration) -> GameSignal {
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

        self.render(ui);
        GameSignal::Continue
    }

    fn restart(&mut self) {
        self.game = Game::new();
        self.phase = Phase::Playing;
        self.gravity_accumulator = Duration::ZERO;
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

    fn render(&self, ui: &mut Context) {
        if ui.width() < 52 || ui.height() < 28 {
            let _ = ui
                .bordered(Border::Rounded)
                .title("Tetris")
                .p(1)
                .gap(1)
                .col(|ui| {
                    ui.text("Terminal too small. Resize to at least 52x28.").fg(Color::Yellow);
                    ui.text("g game select");
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
                ui.text("g game select  ·  r restart  ·  q quit").dim();

                if ui.breakpoint() == Breakpoint::Xs || ui.breakpoint() == Breakpoint::Sm {
                    let _ = ui.container().gap(1).col(|ui| {
                        render_board(ui, &self.game);
                        render_sidebar(ui, &self.game, self.phase);
                    });
                } else {
                    let _ = ui.container().gap(2).row(|ui| {
                        render_board(ui, &self.game);
                        render_sidebar(ui, &self.game, self.phase);
                    });
                }
            });
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

    fn ghost_piece(&self) -> ActivePiece {
        let mut ghost = self.current;
        while !self.collides(ghost.shifted(0, 1)) {
            ghost = ghost.shifted(0, 1);
        }
        ghost
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderCell {
    Empty,
    Ghost,
    Solid(TetrominoKind),
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

fn render_board(ui: &mut Context, game: &Game) {
    let ghost = game.ghost_piece();

    let _ = ui
        .bordered(Border::Rounded)
        .title("Board")
        .p(1)
        .col(|ui| {
            let _ = ui.container().gap(0).col(|ui| {
                for y in 0..BOARD_HEIGHT {
                    let _ = ui.container().gap(0).row(|ui| {
                        for x in 0..BOARD_WIDTH {
                            let render_cell = if let Some(kind) = game.cell_at(x, y) {
                                RenderCell::Solid(kind)
                            } else if ghost
                                .cells()
                                .into_iter()
                                .any(|(cell_x, cell_y)| cell_x == x as i32 && cell_y == y as i32)
                            {
                                RenderCell::Ghost
                            } else {
                                RenderCell::Empty
                            };

                            draw_cell(ui, render_cell);
                        }
                    });
                }
            });
        });
}

fn render_sidebar(ui: &mut Context, game: &Game, phase: Phase) {
    let _ = ui.container().w(24).gap(1).col(|ui| {
        let _ = ui
            .bordered(Border::Rounded)
            .title("Status")
            .p(1)
            .gap(1)
            .col(|ui| {
                let (label, color) = match phase {
                    Phase::Playing => ("Playing", Color::LightGreen),
                    Phase::Paused => ("Paused", Color::LightYellow),
                    Phase::GameOver => ("Game Over", Color::LightRed),
                };

                ui.text(label).bold().fg(color);
                if phase == Phase::GameOver {
                    ui.text("Press r to start again.").dim();
                }
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Stats")
            .p(1)
            .gap(1)
            .col(|ui| {
                let _ = ui.stat("Score", &game.score.to_string());
                let _ = ui.stat("Lines", &game.lines.to_string());
                let _ = ui.stat("Level", &game.level.to_string());
                let _ = ui.stat("Speed", &format!("{} ms", game.gravity_interval().as_millis()));
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Next")
            .p(1)
            .col(|ui| {
                render_next_piece(ui, game.next);
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
                ui.text("g          : game menu").dim();
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
                    let preview_cell = if filled {
                        RenderCell::Solid(kind)
                    } else {
                        RenderCell::Empty
                    };
                    draw_cell(ui, preview_cell);
                }
            });
        }
    });
}

fn draw_cell(ui: &mut Context, cell: RenderCell) {
    let color = match cell {
        RenderCell::Empty => EMPTY_CELL,
        RenderCell::Ghost => GHOST_CELL,
        RenderCell::Solid(kind) => kind.color(),
    };
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

    #[test]
    fn ghost_piece_stops_on_top_of_stack() {
        let mut game = Game::new();
        game.current = ActivePiece {
            kind: TetrominoKind::O,
            rotation: 0,
            x: 3,
            y: 0,
        };
        game.board[10][4] = Some(TetrominoKind::I);
        game.board[10][5] = Some(TetrominoKind::I);

        let ghost = game.ghost_piece();

        assert_eq!(ghost.y, 8);
    }
}
