use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const RECORDS_FILE_NAME: &str = "records.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Records {
    pub tetris: TetrisRecords,
    pub minesweeper: MinesweeperRecords,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TetrisRecords {
    pub high_score: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MinesweeperRecords {
    pub best_time_centis: Option<u64>,
}

pub struct RecordsStore {
    path: PathBuf,
    records: Records,
}

impl RecordsStore {
    pub fn load() -> Self {
        let path = PathBuf::from(RECORDS_FILE_NAME);
        let records = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default();

        Self { path, records }
    }

    pub fn records(&self) -> &Records {
        &self.records
    }

    pub fn update_tetris_high_score(&mut self, score: u32) {
        if score <= self.records.tetris.high_score {
            return;
        }

        self.records.tetris.high_score = score;
        let _ = self.save();
    }

    pub fn update_minesweeper_best_time(&mut self, best_time_centis: Option<u64>) {
        let Some(best_time_centis) = best_time_centis else {
            return;
        };

        if self
            .records
            .minesweeper
            .best_time_centis
            .is_some_and(|current| current <= best_time_centis)
        {
            return;
        }

        self.records.minesweeper.best_time_centis = Some(best_time_centis);
        let _ = self.save();
    }

    fn save(&self) -> io::Result<()> {
        let contents = toml::to_string_pretty(&self.records)
            .map_err(|error| io::Error::other(error.to_string()))?;
        fs::write(&self.path, contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults_missing_fields() {
        let records: Records = toml::from_str("").unwrap();

        assert_eq!(records.tetris.high_score, 0);
        assert_eq!(records.minesweeper.best_time_centis, None);
    }

    #[test]
    fn serialize_tetris_high_score_to_toml() {
        let records = Records {
            tetris: TetrisRecords { high_score: 1200 },
            minesweeper: MinesweeperRecords::default(),
        };

        let toml = toml::to_string(&records).unwrap();

        assert!(toml.contains("[tetris]"));
        assert!(toml.contains("high_score = 1200"));
    }

    #[test]
    fn serialize_minesweeper_best_time_to_toml() {
        let records = Records {
            tetris: TetrisRecords::default(),
            minesweeper: MinesweeperRecords {
                best_time_centis: Some(987),
            },
        };

        let toml = toml::to_string(&records).unwrap();

        assert!(toml.contains("[minesweeper]"));
        assert!(toml.contains("best_time_centis = 987"));
    }
}
