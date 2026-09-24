pub mod ai;
pub mod engine;
pub mod rules;

pub use engine::{
    Action, ActionKind, ApiSnapshot, Difficulty, Discard, EngineError, Game, GameEvent, KongKind,
    Observation, Phase, PrivateState, PublicPlayer, PublicState, RoundResult, WinningDecomposition,
    DEFAULT_BASE, DEFAULT_MONEY, DEFAULT_TAI, HUMAN_SEAT,
};
pub use rules::{
    is_flower, is_honor, is_suited, is_terminal, score_hand, Meld, MeldKind, ScoreInput,
    ScoreOutcome, TaiBreakdown, Tile, WinSource, TAI_NAMES, TAI_VALUES,
};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct WasmGame {
        game: Game,
    }

    #[wasm_bindgen]
    impl WasmGame {
        #[wasm_bindgen(constructor)]
        pub fn new(seed: u32) -> Self {
            Self {
                game: Game::new(seed as u64),
            }
        }

        pub fn configure_ai(&mut self, seat: u8, difficulty: String) -> Result<(), JsValue> {
            let difficulty = Difficulty::parse(&difficulty)
                .ok_or_else(|| JsValue::from_str("未知的電腦難度"))?;
            self.game
                .set_difficulty(seat, difficulty)
                .map_err(|error| JsValue::from_str(&error.to_string()))
        }

        pub fn start(&mut self) -> Result<String, JsValue> {
            self.game
                .start()
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            Ok(self.state_json())
        }

        pub fn state_json(&self) -> String {
            serde_json::to_string(&self.game.snapshot()).unwrap_or_else(|_| "{}".to_string())
        }

        pub fn action_json(&mut self, json: String) -> Result<String, JsValue> {
            self.game
                .apply_json(&json)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            Ok(self.state_json())
        }

        pub fn bot_step_json(&mut self) -> Result<String, JsValue> {
            self.game
                .bot_step()
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            Ok(self.state_json())
        }

        pub fn next_hand_json(&mut self) -> Result<String, JsValue> {
            self.game
                .next_hand()
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            Ok(self.state_json())
        }
    }
}
