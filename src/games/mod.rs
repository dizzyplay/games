use std::time::Duration;

use slt::Context;

pub mod tetris;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameId {
    Tetris,
}

pub struct GameDefinition {
    pub id: GameId,
    pub name: &'static str,
    pub description: &'static str,
}

pub const GAME_CATALOG: [GameDefinition; 1] = [GameDefinition {
    id: GameId::Tetris,
    name: "Tetris",
    description: "Classic falling block puzzle built with superlighttui.",
}];

pub fn catalog() -> &'static [GameDefinition] {
    &GAME_CATALOG
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSignal {
    Continue,
    ReturnToMenu,
}

pub enum RunningGame {
    Tetris(tetris::TetrisGame),
}

impl RunningGame {
    pub fn new(id: GameId) -> Self {
        match id {
            GameId::Tetris => Self::Tetris(tetris::TetrisGame::new()),
        }
    }

    pub fn frame(&mut self, ui: &mut Context, delta: Duration) -> GameSignal {
        match self {
            Self::Tetris(game) => game.frame(ui, delta),
        }
    }
}
