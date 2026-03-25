use super::GameSignal;
use slt::Context;

pub struct MyTetrisGame {
    x: u32,
}

impl MyTetrisGame {
    pub fn new() -> Self {
        Self { x: 0 }
    }
    pub fn render(&mut self, ui: &mut Context) {
        let theme = *ui.theme();
        let _ = ui.bordered(slt::Border::Dashed).col(|ui: &mut Context| {
            self.x += 1;
            ui.text(format!("hello"))
                .ml(self.x)
                .mt(self.x)
                .fg(theme.warning);
        });
    }
    pub fn frame(&mut self, ui: &mut Context) -> GameSignal {
        self.render(ui);
        GameSignal::Continue
    }
}
