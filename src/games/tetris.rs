use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slt::{Align, Border, Color, Context, Justify, KeyCode, Style, Theme};

use super::GameSignal;

const BOARD_WIDTH: usize = 10;
const BOARD_HEIGHT: usize = 20;
const PREVIEW_SIZE: usize = 4;
const CELL_WIDTH: u32 = 2;
const SIDEBAR_WIDTH: u32 = 23;
const SIDEBAR_TEXT_WIDTH: usize = 19;
const GAME_WIDTH: u32 = BOARD_WIDTH as u32 * CELL_WIDTH + SIDEBAR_WIDTH + 7;
const MIN_WIDTH: u32 = GAME_WIDTH + 7;
const MIN_HEIGHT: u32 = BOARD_HEIGHT as u32 + 6;
const LINE_CLEAR_ANIMATION: Duration = Duration::from_millis(220);
const SIMPLE_KICKS: [(i32, i32); 6] = [(0, 0), (0, -1), (-1, 0), (1, 0), (-2, 0), (2, 0)];
const EMPTY_CELL: Color = Color::Indexed(244);
const GHOST_CELL: Color = Color::Indexed(230);
const CLEAR_FLASH_CELL: Color = Color::LightYellow;

type Board = [[Option<TetrominoKind>; BOARD_WIDTH]; BOARD_HEIGHT];

pub struct TetrisGame {
    game: Game,
    phase: Phase,
    gravity_accumulator: Duration,
    lock_pending: bool,
    clear_animation: Option<ClearAnimation>,
    high_score: u32,
}

impl TetrisGame {
    pub fn new(high_score: u32) -> Self {
        Self {
            game: Game::new(),
            phase: Phase::Playing,
            gravity_accumulator: Duration::ZERO,
            lock_pending: false,
            clear_animation: None,
            high_score,
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
            self.advance_gravity(delta);
            if self.clear_animation.is_none() {
                self.handle_input(ui);
            }
        }

        self.render(ui);
        GameSignal::Continue
    }

    fn restart(&mut self) {
        self.game = Game::new();
        self.phase = Phase::Playing;
        self.gravity_accumulator = Duration::ZERO;
        self.lock_pending = false;
        self.clear_animation = None;
        self.high_score = self.high_score.max(self.game.score);
    }

    pub fn high_score(&self) -> u32 {
        self.high_score.max(self.game.score)
    }

    fn handle_input(&mut self, ui: &mut Context) {
        if ui.key('h') || ui.key_code(KeyCode::Left) {
            self.try_adjust_piece(|game| game.try_move(-1, 0));
        }
        if ui.key('l') || ui.key_code(KeyCode::Right) {
            self.try_adjust_piece(|game| game.try_move(1, 0));
        }
        if ui.key('k') || ui.key('x') || ui.key_code(KeyCode::Up) {
            self.try_adjust_piece(Game::try_rotate);
        }
        if ui.key('j') || ui.key_code(KeyCode::Down) {
            if !self.step_down(false) {
                self.phase = Phase::GameOver;
            }
        }
        if ui.key(' ') {
            self.game.hard_drop();
            if !self.resolve_lock() {
                self.phase = Phase::GameOver;
            }
            self.gravity_accumulator = Duration::ZERO;
            self.lock_pending = false;
            return;
        }
    }

    fn advance_gravity(&mut self, delta: Duration) {
        if !self.advance_clear_animation(delta) {
            self.phase = Phase::GameOver;
            return;
        }

        self.gravity_accumulator += delta;
        let interval = self.game.gravity_interval();

        while self.gravity_accumulator >= interval {
            self.gravity_accumulator -= interval;
            if !self.step_down(true) {
                self.phase = Phase::GameOver;
                self.gravity_accumulator = Duration::ZERO;
                break;
            }
        }
    }

    fn try_adjust_piece(&mut self, action: impl FnOnce(&mut Game) -> bool) {
        if action(&mut self.game) {
            self.lock_pending = self.game.is_grounded();
        }
    }

    fn step_down(&mut self, by_gravity: bool) -> bool {
        if self.game.advance_gravity(by_gravity) {
            self.lock_pending = self.game.is_grounded();
            true
        } else if self.lock_pending {
            self.lock_pending = false;
            self.resolve_lock()
        } else {
            self.lock_pending = true;
            true
        }
    }

    fn resolve_lock(&mut self) -> bool {
        self.game.lock_piece();
        let rows = self.game.full_rows();

        if rows.is_empty() {
            self.game.spawn_next_piece()
        } else {
            self.clear_animation = Some(ClearAnimation::new(rows));
            true
        }
    }

    fn advance_clear_animation(&mut self, delta: Duration) -> bool {
        let Some(animation) = &mut self.clear_animation else {
            return true;
        };

        animation.elapsed += delta;
        if animation.elapsed < LINE_CLEAR_ANIMATION {
            return true;
        }

        let rows = animation.rows.clone();
        let cleared = rows.len() as u32;
        self.game.clear_rows(&rows);
        self.game.apply_score(cleared);
        self.clear_animation = None;
        self.game.spawn_next_piece()
    }

    fn render(&self, ui: &mut Context) {
        let theme = *ui.theme();
        let left = ui.width().saturating_sub(GAME_WIDTH) / 2;
        let height = ui.height() as u32;

        if ui.width() < MIN_WIDTH || ui.height() < MIN_HEIGHT {
            let _ = ui
                .bordered(Border::Rounded)
                .title("Tetris")
                .p(1)
                .gap(1)
                .col(|ui| {
                    ui.text(format!(
                        "Terminal too small. Resize to at least {}x{}.",
                        MIN_WIDTH, MIN_HEIGHT
                    ))
                    .fg(theme.warning);
                    ui.text("Score, lines, and level are clipped otherwise.")
                        .fg(theme.text);
                    ui.text("g game select").fg(theme.text_dim);
                    ui.text("q quit").fg(theme.text_dim);
                });
            return;
        }

        let _ = ui.container().h(height).justify(Justify::Center).col(|ui| {
            let _ = ui
                .bordered(Border::Rounded)
                .title("Tetris")
                .w(GAME_WIDTH)
                .ml(left)
                .col(|ui| {
                    ui.text("g game select  ·  r restart  ·  q quit")
                        .fg(theme.text_dim);
                    render_phase_banner(ui, self.phase, theme);

                    let _ = ui.container().gap(1).align(Align::Start).row(|ui| {
                        let _ = ui.container().align_self(Align::Start).col(|ui| {
                            render_board(ui, &self.game, self.clear_animation.as_ref());
                        });
                        let _ = ui.container().align_self(Align::Start).col(|ui| {
                            render_sidebar(ui, &self.game, self.phase, self.high_score(), theme);
                        });
                    });
                });
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
        for (kick_x, kick_y) in SIMPLE_KICKS {
            let candidate = rotated.shifted(kick_x, kick_y);
            if !self.collides(candidate) {
                self.current = candidate;
                return true;
            }
        }
        false
    }

    fn hard_drop(&mut self) -> bool {
        let mut dropped_rows = 0_u32;
        while self.try_move(0, 1) {
            dropped_rows = dropped_rows.saturating_add(1);
        }
        self.score = self.score.saturating_add(dropped_rows.saturating_mul(2));
        true
    }

    fn advance_gravity(&mut self, by_gravity: bool) -> bool {
        if self.try_move(0, 1) {
            if !by_gravity {
                self.score = self.score.saturating_add(1);
            }
            true
        } else {
            false
        }
    }

    fn lock_piece(&mut self) {
        for (x, y) in self.current.cells() {
            if y >= 0 && y < BOARD_HEIGHT as i32 && x >= 0 && x < BOARD_WIDTH as i32 {
                self.board[y as usize][x as usize] = Some(self.current.kind);
            }
        }
    }

    fn spawn_next_piece(&mut self) -> bool {
        self.current = ActivePiece::spawn(self.next);
        self.next = self.randomizer.next();

        !self.collides(self.current)
    }

    fn full_rows(&self) -> Vec<usize> {
        (0..BOARD_HEIGHT)
            .filter(|&y| self.board[y].iter().all(Option::is_some))
            .collect()
    }

    fn clear_rows(&mut self, rows: &[usize]) {
        let mut next_board = empty_board();
        let mut write_y = BOARD_HEIGHT;

        for y in (0..BOARD_HEIGHT).rev() {
            if !rows.contains(&y) {
                write_y -= 1;
                next_board[write_y] = self.board[y];
            }
        }

        self.board = next_board;
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

    fn is_grounded(&self) -> bool {
        self.collides(self.current.shifted(0, 1))
    }
}

struct ClearAnimation {
    rows: Vec<usize>,
    elapsed: Duration,
}

impl ClearAnimation {
    fn new(rows: Vec<usize>) -> Self {
        Self {
            rows,
            elapsed: Duration::ZERO,
        }
    }

    fn flashes_on(&self) -> bool {
        (self.elapsed.as_millis() / 55).is_multiple_of(2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderCell {
    Empty,
    Ghost,
    ClearFlash,
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
            x: spawn_column(),
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
        let next_rotation = (self.rotation + 1) % 4;
        let (old_min_x, old_min_y) = self.kind.min_offset(self.rotation);
        let (new_min_x, new_min_y) = self.kind.min_offset(next_rotation);

        Self {
            rotation: next_rotation,
            x: self.x + old_min_x - new_min_x,
            y: self.y + old_min_y - new_min_y,
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

    fn min_offset(self, rotation: usize) -> (i32, i32) {
        self.offsets(rotation)
            .into_iter()
            .fold((i32::MAX, i32::MAX), |(min_x, min_y), (x, y)| {
                (min_x.min(x), min_y.min(y))
            })
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

fn render_board(ui: &mut Context, game: &Game, clear_animation: Option<&ClearAnimation>) {
    let ghost = game.ghost_piece();
    let flashing_rows = clear_animation
        .filter(|animation| animation.flashes_on())
        .map(|animation| animation.rows.as_slice());

    let _ = ui.bordered(Border::Double).col(|ui| {
        let _ = ui.container().gap(0).col(|ui| {
            for y in 0..BOARD_HEIGHT {
                let _ = ui.container().gap(0).row(|ui| {
                    for x in 0..BOARD_WIDTH {
                        let render_cell = if flashing_rows.is_some_and(|rows| rows.contains(&y)) {
                            RenderCell::ClearFlash
                        } else if let Some(kind) = game.cell_at(x, y) {
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

fn render_sidebar(ui: &mut Context, game: &Game, phase: Phase, high_score: u32, theme: Theme) {
    let _ = ui.container().w(SIDEBAR_WIDTH).gap(1).col(|ui| {
        let _ = ui
            .bordered(Border::Rounded)
            .title("Next")
            .w(SIDEBAR_WIDTH)
            .p(1)
            .col(|ui| {
                render_next_piece(ui, game.next);
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Stats")
            .w(SIDEBAR_WIDTH)
            .p(1)
            .gap(0)
            .col(|ui| {
                ui.text(format_stat_line("Top Score", high_score))
                    .bold()
                    .fg(theme.warning);
                ui.text(format_stat_line("Score", game.score))
                    .bold()
                    .fg(theme.primary);
                ui.text(format_stat_line("Lines", game.lines))
                    .bold()
                    .fg(theme.primary);
                ui.text(format_stat_line("Level", game.level))
                    .bold()
                    .fg(theme.primary);
            });

        let _ = ui.container().title("control").gap(0).col(|ui| {
            ui.text("h j k l - move ").fg(theme.text_dim);
            ui.text("space - drop").fg(theme.text_dim);
            if phase == Phase::Paused {
                ui.text("p resume").fg(theme.warning);
            } else if phase == Phase::GameOver {
                ui.text("r restart").fg(theme.error);
            } else {
                ui.text("p pause  g menu").fg(theme.text_dim);
            }
            ui.text("q quit").fg(theme.text_dim);
        });
    });
}

fn format_stat_line(label: &str, value: u32) -> String {
    let value = value.to_string();
    let padding = SIDEBAR_TEXT_WIDTH.saturating_sub(label.len() + value.len());
    format!("{label}{}{value}", " ".repeat(padding))
}

fn render_phase_banner(ui: &mut Context, phase: Phase, theme: Theme) {
    match phase {
        Phase::Playing => ui.text(" "),
        Phase::Paused => ui
            .text("Paused  ·  press p to resume")
            .bold()
            .fg(theme.warning),
        Phase::GameOver => ui
            .text("Game Over  ·  press r to restart")
            .bold()
            .fg(theme.error),
    };
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
    match cell {
        RenderCell::Empty => ui.styled("· ", Style::new().fg(EMPTY_CELL)),
        RenderCell::Ghost => ui.styled("░░", Style::new().fg(GHOST_CELL)),
        RenderCell::ClearFlash => ui.styled("██", Style::new().fg(CLEAR_FLASH_CELL)),
        RenderCell::Solid(kind) => ui.styled("██", Style::new().fg(kind.color())),
    };
}

const fn spawn_column() -> i32 {
    BOARD_WIDTH.saturating_sub(PREVIEW_SIZE) as i32 / 2
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
    use slt::TestBackend;

    #[test]
    fn line_clear_removes_full_rows() {
        let mut game = Game::new();
        game.board[BOARD_HEIGHT - 1] = [Some(TetrominoKind::I); BOARD_WIDTH];
        game.board[BOARD_HEIGHT - 2][0] = Some(TetrominoKind::O);
        let full_rows = game.full_rows();

        game.clear_rows(&full_rows);

        assert_eq!(full_rows, vec![BOARD_HEIGHT - 1]);
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
            x: BOARD_WIDTH.saturating_sub(3) as i32,
            y: 0,
        };

        assert!(game.try_rotate());
        assert!(game.current.x <= BOARD_WIDTH.saturating_sub(3) as i32);
    }

    #[test]
    fn rotate_prefers_vertical_kick_while_keeping_left_edge_stable() {
        let mut game = Game::new();
        game.current = ActivePiece {
            kind: TetrominoKind::T,
            rotation: 0,
            x: spawn_column(),
            y: BOARD_HEIGHT.saturating_sub(2) as i32,
        };
        let original_left_edge = game
            .current
            .cells()
            .into_iter()
            .map(|(x, _)| x)
            .min()
            .unwrap();

        assert!(game.try_rotate());
        let rotated_left_edge = game
            .current
            .cells()
            .into_iter()
            .map(|(x, _)| x)
            .min()
            .unwrap();

        assert_eq!(rotated_left_edge, original_left_edge);
        assert_eq!(game.current.y, BOARD_HEIGHT.saturating_sub(3) as i32);
    }

    #[test]
    fn free_rotation_keeps_piece_left_edge_stable() {
        let mut game = Game::new();
        game.current = ActivePiece {
            kind: TetrominoKind::Z,
            rotation: 0,
            x: spawn_column(),
            y: 5,
        };
        let original_left_edge = game
            .current
            .cells()
            .into_iter()
            .map(|(x, _)| x)
            .min()
            .unwrap();

        assert!(game.try_rotate());

        let rotated_left_edge = game
            .current
            .cells()
            .into_iter()
            .map(|(x, _)| x)
            .min()
            .unwrap();
        assert_eq!(rotated_left_edge, original_left_edge);
    }

    #[test]
    fn spawn_failure_returns_game_over() {
        let mut game = Game::new();
        let spawn_x = spawn_column() as usize;
        for y in 0..4 {
            for x in spawn_x..(spawn_x + PREVIEW_SIZE).min(BOARD_WIDTH) {
                game.board[y][x] = Some(TetrominoKind::T);
            }
        }
        game.current = ActivePiece {
            kind: TetrominoKind::O,
            rotation: 0,
            x: spawn_column(),
            y: BOARD_HEIGHT.saturating_sub(2) as i32,
        };
        game.lock_piece();

        assert!(!game.spawn_next_piece());
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
    fn hard_drop_adds_cell_based_score() {
        let mut game = Game::new();

        game.hard_drop();

        assert!(game.score > 0);
    }

    #[test]
    fn ghost_piece_stops_on_top_of_stack() {
        let mut game = Game::new();
        game.current = ActivePiece {
            kind: TetrominoKind::O,
            rotation: 0,
            x: spawn_column(),
            y: 0,
        };
        let stack_y = BOARD_HEIGHT / 4;
        let left_x = (spawn_column() + 1) as usize;
        let right_x = (spawn_column() + 2) as usize;
        game.board[stack_y][left_x] = Some(TetrominoKind::I);
        game.board[stack_y][right_x] = Some(TetrominoKind::I);

        let ghost = game.ghost_piece();

        assert_eq!(ghost.y, stack_y as i32 - 2);
    }

    #[test]
    fn grounded_piece_waits_before_locking() {
        let mut tetris = TetrisGame::new(0);
        tetris.game.current = ActivePiece {
            kind: TetrominoKind::O,
            rotation: 0,
            x: spawn_column(),
            y: BOARD_HEIGHT.saturating_sub(2) as i32,
        };
        let interval = tetris.game.gravity_interval();

        tetris.advance_gravity(interval);

        assert_eq!(tetris.phase, Phase::Playing);
        assert_eq!(tetris.game.current.kind, TetrominoKind::O);
        assert!(tetris.game.board.iter().flatten().all(Option::is_none));
        assert!(tetris.lock_pending);
    }

    #[test]
    fn grounded_piece_locks_on_second_gravity_tick() {
        let mut tetris = TetrisGame::new(0);
        tetris.game.current = ActivePiece {
            kind: TetrominoKind::O,
            rotation: 0,
            x: spawn_column(),
            y: BOARD_HEIGHT.saturating_sub(2) as i32,
        };
        let interval = tetris.game.gravity_interval();

        tetris.advance_gravity(interval);
        tetris.advance_gravity(interval);

        let left_x = (spawn_column() + 1) as usize;
        let right_x = (spawn_column() + 2) as usize;
        assert_eq!(
            tetris.game.board[BOARD_HEIGHT - 1][left_x],
            Some(TetrominoKind::O)
        );
        assert_eq!(
            tetris.game.board[BOARD_HEIGHT - 1][right_x],
            Some(TetrominoKind::O)
        );
        assert!(!tetris.lock_pending);
    }

    #[test]
    fn hard_drop_spawns_next_piece_immediately() {
        let mut tetris = TetrisGame::new(0);
        tetris.game.current = ActivePiece {
            kind: TetrominoKind::I,
            rotation: 0,
            x: spawn_column(),
            y: 0,
        };
        let starting_next = tetris.game.next;

        tetris.game.hard_drop();
        assert!(tetris.resolve_lock());

        assert_eq!(tetris.game.current.kind, starting_next);
        assert_ne!(tetris.game.next, starting_next);
        assert!(tetris.game.board.iter().flatten().any(Option::is_some));
    }

    #[test]
    fn spawn_position_tracks_board_width() {
        let piece = ActivePiece::spawn(TetrominoKind::I);

        assert_eq!(piece.x, spawn_column());
        assert_eq!(piece.x, BOARD_WIDTH.saturating_sub(PREVIEW_SIZE) as i32 / 2);
    }

    #[test]
    fn render_shows_stats_when_terminal_is_large_enough() {
        let mut backend = TestBackend::new(MIN_WIDTH, MIN_HEIGHT);
        let tetris = TetrisGame::new(1234);

        backend.render(|ui| tetris.render(ui));

        backend.assert_contains("Score");
        backend.assert_contains("Lines");
        backend.assert_contains("Level");
        backend.assert_contains("1234");
    }

    #[test]
    fn render_shows_resize_hint_when_terminal_is_too_small() {
        let mut backend = TestBackend::new(MIN_WIDTH - 1, MIN_HEIGHT);
        let tetris = TetrisGame::new(0);

        backend.render(|ui| tetris.render(ui));

        backend.assert_contains("Terminal too small");
        backend.assert_contains("Score, lines, and level are clipped otherwise.");
    }
}
