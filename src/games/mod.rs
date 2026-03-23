use std::time::Duration;

use slt::Context;

use crate::records::{Records, RecordsStore};

pub mod minesweeper;
pub mod tetris;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameId {
    Tetris,
    Minesweeper,
}

pub struct GameDefinition {
    pub id: GameId,
    pub name: &'static str,
    pub description: &'static str,
}

pub const GAME_CATALOG: [GameDefinition; 2] = [
    GameDefinition {
        id: GameId::Tetris,
        name: "Tetris",
        description: "테트리스",
    },
    GameDefinition {
        id: GameId::Minesweeper,
        name: "Minesweeper",
        description: "지뢰찾기",
    },
];

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
    Minesweeper(minesweeper::MinesweeperGame),
}

impl RunningGame {
    pub fn new(id: GameId, records: &Records) -> Self {
        match id {
            GameId::Tetris => Self::Tetris(tetris::TetrisGame::new(records.tetris.high_score)),
            GameId::Minesweeper => Self::Minesweeper(minesweeper::MinesweeperGame::new(
                records.minesweeper.best_time_centis,
            )),
        }
    }

    pub fn frame(&mut self, ui: &mut Context, delta: Duration) -> GameSignal {
        match self {
            Self::Tetris(game) => game.frame(ui, delta),
            Self::Minesweeper(game) => game.frame(ui, delta),
        }
    }

    pub fn sync_records(&self, store: &mut RecordsStore) {
        match self {
            Self::Tetris(game) => store.update_tetris_high_score(game.high_score()),
            Self::Minesweeper(game) => store.update_minesweeper_best_time(game.best_time_centis()),
        }
    }
}
