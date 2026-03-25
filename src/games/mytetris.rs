use super::GameSignal;
use slt::{Context, Style};

pub struct MyTetrisGame {
    game: GameState,
}
pub struct GameState {
    x: i16,
    y: i16,
}

impl GameState {
    pub fn new() -> Self {
        Self { x: 0, y: 0 }
    }
    pub fn move_block_delta(&mut self, x: i16, y: i16) {
        if self.x + x > 0 {
            self.x += x;
        }
        if (self.y + y) > 0 {
            self.y += y;
        }
    }
    pub fn block_position(&self) -> (u32, u32) {
        (self.x as u32, self.y as u32)
    }
}

impl MyTetrisGame {
    pub fn new() -> Self {
        Self {
            game: GameState::new(),
        }
    }
    pub fn render(&mut self, ui: &mut Context) {
        let theme = *ui.theme();
        if ui.key('h') {
            self.game.move_block_delta(-1, 0)
        } else if ui.key('l') {
            self.game.move_block_delta(1, 0);
        } else if ui.key('k') {
            self.game.move_block_delta(0, -1);
        } else if ui.key('j') {
            self.game.move_block_delta(0, 1);
        }

        let x = self.game.block_position().0;
        let y = self.game.block_position().1;
        let _ = ui
            .bordered(slt::Border::Single)
            .w(20)
            .h(10)
            .draw(move |buf, rect| {
                buf.set_char(x, y, 'H', Style::new());
            });
    }
    pub fn frame(&mut self, ui: &mut Context) -> GameSignal {
        if ui.key('z') {
            return GameSignal::ReturnToMenu;
        }
        self.render(ui);
        GameSignal::Continue
    }
}
