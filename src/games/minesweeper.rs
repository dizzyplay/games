use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slt::{Align, Border, Color, Context, KeyCode, Style, Theme};

use super::GameSignal;

const BOARD_WIDTH: usize = 30;
const BOARD_HEIGHT: usize = 30;
const MINE_COUNT: usize = 112;
const SAFE_CELL_COUNT: usize = BOARD_WIDTH * BOARD_HEIGHT - MINE_COUNT;
const CELL_WIDTH: u32 = 2;
const SIDEBAR_WIDTH: u32 = 24;
const GAME_WIDTH: u32 = BOARD_WIDTH as u32 * CELL_WIDTH + SIDEBAR_WIDTH + 7;
const MIN_WIDTH: u32 = GAME_WIDTH + 7;
const MIN_HEIGHT: u32 = BOARD_HEIGHT as u32 + 6;
type Board = [[Cell; BOARD_WIDTH]; BOARD_HEIGHT];

pub struct MinesweeperGame {
    game: Game,
    phase: Phase,
    elapsed: Duration,
    best_time: Option<Duration>,
}

impl MinesweeperGame {
    pub fn new(best_time_centis: Option<u64>) -> Self {
        Self {
            game: Game::new(),
            phase: Phase::Playing,
            elapsed: Duration::ZERO,
            best_time: best_time_centis.map(duration_from_centis),
        }
    }

    pub fn frame(&mut self, ui: &mut Context, delta: Duration) -> GameSignal {
        if ui.key('r') {
            self.restart();
        }

        self.handle_cursor_input(ui);

        if self.phase == Phase::Playing {
            if self.game.mines_armed {
                self.elapsed += delta;
            }
            self.handle_action_input(ui);
        }

        self.render(ui);
        GameSignal::Continue
    }

    pub fn best_time_centis(&self) -> Option<u64> {
        self.best_time.map(duration_to_centis)
    }

    fn restart(&mut self) {
        self.game = Game::new();
        self.phase = Phase::Playing;
        self.elapsed = Duration::ZERO;
    }

    fn handle_cursor_input(&mut self, ui: &mut Context) {
        if ui.key('h') || ui.key_code(KeyCode::Left) {
            self.game.move_cursor(-1, 0);
        }
        if ui.key('l') || ui.key_code(KeyCode::Right) {
            self.game.move_cursor(1, 0);
        }
        if ui.key('k') || ui.key_code(KeyCode::Up) {
            self.game.move_cursor(0, -1);
        }
        if ui.key('j') || ui.key_code(KeyCode::Down) {
            self.game.move_cursor(0, 1);
        }
    }

    fn handle_action_input(&mut self, ui: &mut Context) {
        if ui.key('f') {
            self.game.toggle_flag();
        }

        if ui.key_code(KeyCode::Enter) || ui.key(' ') {
            self.phase = match self.game.reveal() {
                TurnResult::Continue | TurnResult::NoChange => Phase::Playing,
                TurnResult::Lost => Phase::Lost,
                TurnResult::Won => {
                    self.best_time = match self.best_time {
                        Some(best) if best <= self.elapsed => Some(best),
                        _ => Some(self.elapsed),
                    };
                    Phase::Won
                }
            };
        }
    }

    fn render(&self, ui: &mut Context) {
        let theme = *ui.theme();

        if ui.width() < MIN_WIDTH || ui.height() < MIN_HEIGHT {
            let _ = ui
                .bordered(Border::Rounded)
                .title("Minesweeper")
                .p(1)
                .gap(1)
                .col(|ui| {
                    ui.text(format!(
                        "Terminal too small. Resize to at least {}x{}.",
                        MIN_WIDTH, MIN_HEIGHT
                    ))
                    .fg(Color::Yellow);
                    ui.text("Enter open  ·  f flag");
                    ui.text("g game select  ·  q quit");
                });
            return;
        }

        let left = ui.width().saturating_sub(GAME_WIDTH) / 2;

        let _ = ui
            .bordered(Border::Rounded)
            .title("Minesweeper")
            .w(GAME_WIDTH)
            .ml(left)
            .col(|ui| {
                ui.text("g game select  ·  r restart  ·  q quit").dim();
                render_phase_banner(ui, self.phase);

                let _ = ui.container().gap(1).align(Align::Start).row(|ui| {
                    let _ = ui.container().align_self(Align::Start).col(|ui| {
                        render_board(ui, &self.game, self.phase, theme);
                    });
                    let _ = ui.container().align_self(Align::Start).col(|ui| {
                        render_sidebar(ui, self, theme);
                    });
                });
            });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnResult {
    NoChange,
    Continue,
    Won,
    Lost,
}

struct Game {
    board: Board,
    cursor_x: usize,
    cursor_y: usize,
    revealed_safe_cells: usize,
    flag_count: usize,
    mines_armed: bool,
    exploded: Option<(usize, usize)>,
    randomizer: Randomizer,
}

impl Game {
    fn new() -> Self {
        Self {
            board: empty_board(),
            cursor_x: BOARD_WIDTH / 2,
            cursor_y: BOARD_HEIGHT / 2,
            revealed_safe_cells: 0,
            flag_count: 0,
            mines_armed: false,
            exploded: None,
            randomizer: Randomizer::new(),
        }
    }

    fn move_cursor(&mut self, dx: i32, dy: i32) {
        self.cursor_x = clamp_index(self.cursor_x, dx, BOARD_WIDTH);
        self.cursor_y = clamp_index(self.cursor_y, dy, BOARD_HEIGHT);
    }

    fn toggle_flag(&mut self) {
        let cell = &mut self.board[self.cursor_y][self.cursor_x];
        match cell.state {
            CellState::Hidden => {
                cell.state = CellState::Flagged;
                self.flag_count += 1;
            }
            CellState::Flagged => {
                cell.state = CellState::Hidden;
                self.flag_count -= 1;
            }
            CellState::Revealed => {}
        }
    }

    fn reveal(&mut self) -> TurnResult {
        self.reveal_at(self.cursor_x, self.cursor_y)
    }

    fn reveal_at(&mut self, x: usize, y: usize) -> TurnResult {
        let state = self.board[y][x].state;
        if state == CellState::Flagged || state == CellState::Revealed {
            return TurnResult::NoChange;
        }

        if !self.mines_armed {
            self.place_mines((x, y));
        }

        if self.board[y][x].has_mine {
            self.board[y][x].state = CellState::Revealed;
            self.exploded = Some((x, y));
            return TurnResult::Lost;
        }

        self.reveal_region(x, y);

        if self.revealed_safe_cells == SAFE_CELL_COUNT {
            TurnResult::Won
        } else {
            TurnResult::Continue
        }
    }

    fn reveal_region(&mut self, x: usize, y: usize) {
        let mut stack = vec![(x, y)];

        while let Some((cell_x, cell_y)) = stack.pop() {
            let cell = &mut self.board[cell_y][cell_x];
            if cell.state != CellState::Hidden || cell.has_mine {
                continue;
            }

            cell.state = CellState::Revealed;
            self.revealed_safe_cells += 1;

            if cell.adjacent_mines != 0 {
                continue;
            }

            for neighbor_y in cell_y.saturating_sub(1)..=(cell_y + 1).min(BOARD_HEIGHT - 1) {
                for neighbor_x in cell_x.saturating_sub(1)..=(cell_x + 1).min(BOARD_WIDTH - 1) {
                    if neighbor_x == cell_x && neighbor_y == cell_y {
                        continue;
                    }
                    if self.board[neighbor_y][neighbor_x].state == CellState::Hidden {
                        stack.push((neighbor_x, neighbor_y));
                    }
                }
            }
        }
    }

    fn place_mines(&mut self, safe_cell: (usize, usize)) {
        let mut candidates = Vec::with_capacity(BOARD_WIDTH * BOARD_HEIGHT - 1);

        for y in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH {
                if (x, y) != safe_cell {
                    candidates.push((x, y));
                }
            }
        }

        self.randomizer.shuffle(&mut candidates);

        for (x, y) in candidates.into_iter().take(MINE_COUNT) {
            self.board[y][x].has_mine = true;
        }

        self.recompute_adjacent_mines();
        self.mines_armed = true;
    }

    fn recompute_adjacent_mines(&mut self) {
        for y in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH {
                let mut count = 0_u8;
                for neighbor_y in y.saturating_sub(1)..=(y + 1).min(BOARD_HEIGHT - 1) {
                    for neighbor_x in x.saturating_sub(1)..=(x + 1).min(BOARD_WIDTH - 1) {
                        if neighbor_x == x && neighbor_y == y {
                            continue;
                        }
                        if self.board[neighbor_y][neighbor_x].has_mine {
                            count += 1;
                        }
                    }
                }
                self.board[y][x].adjacent_mines = count;
            }
        }
    }

    fn remaining_safe_cells(&self) -> usize {
        SAFE_CELL_COUNT - self.revealed_safe_cells
    }

    fn mines_left(&self) -> i32 {
        MINE_COUNT as i32 - self.flag_count as i32
    }

    fn render_cell(&self, x: usize, y: usize, phase: Phase) -> RenderCell {
        let cell = self.board[y][x];
        let is_exploded = self.exploded == Some((x, y));

        if is_exploded {
            return RenderCell::Exploded;
        }

        if phase == Phase::Lost && cell.has_mine {
            return RenderCell::Mine;
        }

        match cell.state {
            CellState::Hidden => RenderCell::Hidden,
            CellState::Flagged => {
                if phase == Phase::Lost && !cell.has_mine {
                    RenderCell::WrongFlag
                } else {
                    RenderCell::Flagged
                }
            }
            CellState::Revealed => {
                if cell.adjacent_mines == 0 {
                    RenderCell::Empty
                } else {
                    RenderCell::Number(cell.adjacent_mines)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
struct Cell {
    has_mine: bool,
    adjacent_mines: u8,
    state: CellState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum CellState {
    #[default]
    Hidden,
    Revealed,
    Flagged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderCell {
    Hidden,
    Empty,
    Number(u8),
    Flagged,
    WrongFlag,
    Mine,
    Exploded,
}

struct Randomizer {
    state: u64,
}

impl Randomizer {
    fn new() -> Self {
        Self { state: seed() }
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            values.swap(i, j);
        }
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

fn render_board(ui: &mut Context, game: &Game, phase: Phase, theme: Theme) {
    let _ = ui.bordered(Border::Double).col(|ui| {
        let _ = ui.container().gap(0).col(|ui| {
            for y in 0..BOARD_HEIGHT {
                let _ = ui.container().gap(0).row(|ui| {
                    for x in 0..BOARD_WIDTH {
                        draw_cell(
                            ui,
                            game.render_cell(x, y, phase),
                            x == game.cursor_x && y == game.cursor_y,
                            theme,
                        );
                    }
                });
            }
        });
    });
}

fn render_sidebar(ui: &mut Context, game: &MinesweeperGame, theme: Theme) {
    let _ = ui.container().w(SIDEBAR_WIDTH).gap(1).col(|ui| {
        let _ = ui
            .bordered(Border::Rounded)
            .title("Stats")
            .p(1)
            .gap(0)
            .col(|ui| {
                let _ = ui.row(|ui| {
                    ui.text("Time").dim();
                    ui.spacer();
                    ui.timer_display(game.elapsed).bold();
                });
                let _ = ui.row(|ui| {
                    ui.text("Best").dim();
                    ui.spacer();
                    match game.best_time {
                        Some(best_time) => {
                            ui.timer_display(best_time).bold().fg(Color::LightYellow);
                        }
                        None => {
                            ui.text("--:--.--").dim();
                        }
                    };
                });
                let _ = ui.row(|ui| {
                    ui.text("Mines Left").dim();
                    ui.spacer();
                    ui.text(game.game.mines_left().to_string()).bold();
                });
                let _ = ui.row(|ui| {
                    ui.text("Safe Left").dim();
                    ui.spacer();
                    ui.text(game.game.remaining_safe_cells().to_string()).bold();
                });
            });

        let _ = ui
            .bordered(Border::Rounded)
            .title("Status")
            .p(1)
            .gap(0)
            .col(|ui| {
                ui.text("30x30 field").bold();
                ui.text("112 mines").bold();
                ui.separator();
                ui.text("First reveal is always safe.").dim();
                match game.phase {
                    Phase::Playing => ui.text("Clear every safe tile.").fg(theme.primary),
                    Phase::Won => ui.text("Board cleared.").fg(theme.success),
                    Phase::Lost => ui.text("Mine triggered.").fg(theme.error),
                };
            });

        let _ = ui.container().title("control").gap(0).col(|ui| {
            ui.text("h j k l - move").dim();
            ui.text("enter/space - open").dim();
            ui.text("f - flag").dim();
            if game.phase == Phase::Playing {
                ui.text("r restart  g menu").dim();
            } else {
                ui.text("r restart").fg(Color::LightYellow);
            }
            ui.text("q quit").dim();
        });
    });
}

fn render_phase_banner(ui: &mut Context, phase: Phase) {
    match phase {
        Phase::Playing => ui.text(" "),
        Phase::Won => ui
            .text("Clear  ·  press r to restart")
            .bold()
            .fg(Color::LightGreen),
        Phase::Lost => ui
            .text("Boom  ·  press r to restart")
            .bold()
            .fg(Color::LightRed),
    };
}

fn draw_cell(ui: &mut Context, cell: RenderCell, selected: bool, theme: Theme) {
    let (content, mut style) = match cell {
        RenderCell::Hidden => ("·", Style::new().fg(theme.text_dim)),
        RenderCell::Empty => (" ", Style::new().fg(theme.text_dim)),
        RenderCell::Number(count) => (number_text(count), Style::new().fg(number_color(count))),
        RenderCell::Flagged => ("⚑", Style::new().fg(theme.warning).bold()),
        RenderCell::WrongFlag => ("x", Style::new().fg(theme.error).bold()),
        RenderCell::Mine => ("*", Style::new().fg(theme.error).bold()),
        RenderCell::Exploded => ("*", Style::new().fg(theme.error).bold()),
    };

    if selected {
        style = style.reversed().bold();
    }

    ui.styled(format!("{content} "), style);
}

fn number_text(count: u8) -> &'static str {
    match count {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        _ => " ",
    }
}

fn number_color(count: u8) -> Color {
    match count {
        1 => Color::LightCyan,
        2 => Color::LightGreen,
        3 => Color::LightRed,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        8 => Color::Indexed(250),
        _ => Color::White,
    }
}

fn clamp_index(value: usize, delta: i32, limit: usize) -> usize {
    match delta.cmp(&0) {
        std::cmp::Ordering::Less => value.saturating_sub(1),
        std::cmp::Ordering::Greater => (value + 1).min(limit - 1),
        std::cmp::Ordering::Equal => value,
    }
}

fn duration_from_centis(centis: u64) -> Duration {
    Duration::from_millis(centis.saturating_mul(10))
}

fn duration_to_centis(duration: Duration) -> u64 {
    (duration.as_millis() / 10) as u64
}

fn empty_board() -> Board {
    [[Cell::default(); BOARD_WIDTH]; BOARD_HEIGHT]
}

fn seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);

    nanos ^ 0x517C_C1B7_3D29_A4EF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_reveal_never_places_mine_under_cursor() {
        let mut game = Game::new();

        let result = game.reveal_at(4, 4);

        assert_eq!(result, TurnResult::Continue);
        assert!(game.mines_armed);
        assert!(!game.board[4][4].has_mine);
        assert_eq!(count_mines(&game), MINE_COUNT);
    }

    #[test]
    fn zero_reveal_spreads_across_safe_region() {
        let mut game = Game::new();
        game.mines_armed = true;
        game.board[BOARD_HEIGHT - 1][BOARD_WIDTH - 1].has_mine = true;
        game.recompute_adjacent_mines();

        let result = game.reveal_at(0, 0);

        assert_eq!(result, TurnResult::Continue);
        assert!(game.revealed_safe_cells > 1);
        assert_eq!(game.board[0][0].state, CellState::Revealed);
    }

    #[test]
    fn toggle_flag_marks_and_unmarks_hidden_cell() {
        let mut game = Game::new();
        game.cursor_x = 2;
        game.cursor_y = 3;

        game.toggle_flag();
        assert_eq!(game.board[3][2].state, CellState::Flagged);
        assert_eq!(game.flag_count, 1);

        game.toggle_flag();
        assert_eq!(game.board[3][2].state, CellState::Hidden);
        assert_eq!(game.flag_count, 0);
    }

    #[test]
    fn revealing_mine_returns_loss() {
        let mut game = Game::new();
        game.mines_armed = true;
        game.board[1][1].has_mine = true;
        game.recompute_adjacent_mines();

        let result = game.reveal_at(1, 1);

        assert_eq!(result, TurnResult::Lost);
        assert_eq!(game.exploded, Some((1, 1)));
        assert_eq!(game.board[1][1].state, CellState::Revealed);
    }

    fn count_mines(game: &Game) -> usize {
        game.board
            .iter()
            .flatten()
            .filter(|cell| cell.has_mine)
            .count()
    }
}
