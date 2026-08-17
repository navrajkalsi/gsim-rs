//! # GSim Configuration
//!
//! Configuration file parser.
//! Reads and parses **JSON** config file at a provided file path.
//!
//! ## Example
//! The following is an example of a valid config file:
//! ```text
//! {
//!  "units": "metric",
//!  "stock": {
//!    "x": 500,
//!    "y": 500,
//!    "z": 500
//!  },
//!  "zero_pos": {
//!    "x": 0,
//!    "y": 0,
//!    "z": 0
//!  },
//!  "start_pos": {
//!    "x": 250,
//!    "y": 250,
//!    "z": 750
//!  },
//!  "tools": [
//!    {
//!      "number": 1,
//!      "diameter": 5,
//!      "length": 10
//!    },
//!    {
//!      "number": 2,
//!      "diameter": 5,
//!      "length": 10
//!    }
//!  ],
//!  "default_tool": {
//!    "number": 0,
//!    "diameter": 10,
//!    "length": 20
//!  }
//! }
//! ```
//! - Treats every dimension in the `metric` system.
//! - Creates a cuboid shaped stock, with each side measuring `500mm`.
//! - Does not offset the reference-point of the stock, and sets it as the `zero_pos`.
//!   The reference point of a cuboid stock is the **left-bottom-near** point.
//! - Starts the simulation at `start_pos`, which is offset from the reference point. Here, it
//!   will start at middle of **X** and **Y** of the stock and **250mm** above the stock.
//! - Creates two tools(numbered `1` & `2`), each with `diameter` `5mm` and `length` `10mm`.
//! - Creates a default tool config, also with `diameter` `10mm` and `length` `20mm`.
//!
//! ## Restrictions
//! - Any **excess elements** will be rejected.
//! - `units` can only have two possible values: `imperial` or `metric`.
//! - Every stock dimension **must** be positive and non-zero.
//! - Each tool `diameter` and `length` **must** be positive and non-zero.

use crate::points::Point;
use serde::Deserialize;
use std::str::FromStr;

/// Program configuration at start.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Unit system applied to all dimensional values (e.g. `stock_size`, `tool_length`).
    /// [`Machine`](crate::machine) will also be configured with this system.
    pub units: Unit,

    /// Stock dimensions.
    pub stock: Point,

    /// Work offset zero position.
    /// This is relative to a **stock reference point**.
    /// Reference point is at `0.0` for each axis (**left-bottom-near**).
    pub zero_pos: Point,

    /// Start position at the beginning of the program.
    /// This is relative to **stock reference point**.
    /// Reference point is at `0.0` for each axis (**left-bottom-near**).
    pub start_pos: Point,

    /// Collection of tool configurations to be used during G-code execution.
    pub tools: Vec<ToolConfig>,

    /// Default tool configuration to be used when no tool is selected or the selected tool number
    /// is not found in [`Self::tools`].
    pub default_tool: ToolConfig,
}

impl Config {
    /// Constructs a config by attempting to read a JSON file and then parse it.
    ///
    /// For additional information checkout [`Self::from_str`].
    ///
    /// # Errors:
    /// - [`ConfigError::IO`] -- Could not read the file at provided path.
    /// - [`ConfigError::Parse`] -- Could not parse the provided slice.
    /// - [`ConfigError::StockNonPositive`] -- At least one of the stock axis was zero or negative.
    /// - [`ConfigError::ToolNonPositive`] -- At least one of the tools has zero or negative
    ///   diameter or length.
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        Self::from_str(
            std::fs::read_to_string(path)
                .map_err(|e| ConfigError::IO(e, path.to_owned()))?
                .as_str(),
        )
    }
}

impl FromStr for Config {
    type Err = ConfigError;

    /// Constructs a config by attempting to parse a provided string slice.
    ///
    /// # Errors:
    /// - [`ConfigError::Parse`] -- Could not parse the provided slice.
    /// - [`ConfigError::StockNonPositive`] -- At least one of the stock dimension was zero or negative.
    /// - [`ConfigError::ToolNonPositive`] -- At least one of the tools has zero or negative
    ///   diameter or length.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret: Self = serde_json::from_str(s)?;

        // make sure stock size and tool diameter and length are positive and non zero
        if ret.stock.x.is_sign_negative()
            || ret.stock.y.is_sign_negative()
            || ret.stock.z.is_sign_negative()
        {
            return Err(ConfigError::StockNonPositive);
        }

        for tool in &ret.tools {
            tool.validate()?
        }

        ret.default_tool.validate()?;

        Ok(ret)
    }
}

/// Possible unit standards for dimensional values.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Metric,
    Imperial,
}

/// Tool configuration.
#[derive(Clone, Debug, Copy, Deserialize, PartialEq)]
pub struct ToolConfig {
    /// Number of the tool.
    /// This is denoted with a `T` code in G-code.
    pub number: u32,

    /// Diameter of the tool to render when `self.number` tool is activated.
    /// This is guaranteed to be positive and non zero.
    pub diameter: f32,

    /// Length of the tool to render when `self.number` tool is activated.
    /// This is guaranteed to be positive and non zero.
    pub length: f32,
}

impl ToolConfig {
    /// Verifies the tool to have positive diameter and length.
    ///
    /// Returns [`ConfigError::ToolNonPositive`] if the tool has zero or negative diameter or length.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.diameter.is_sign_negative() || self.length.is_sign_negative() {
            Err(ConfigError::ToolNonPositive(self.number))
        } else {
            Ok(())
        }
    }
}

/// Possible errors that can happen during [`Config`] construction.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Failed to read the config file.
    #[error("failed to read file at '{1}'")]
    IO(#[source] std::io::Error, String),

    /// Failed to parse the config file as JSON.
    #[error("failed to parse JSON")]
    Parse(#[from] serde_json::Error),

    /// Stock dimensions are not all positive.
    #[error("a stock dimension is either negative or zero")]
    StockNonPositive,

    /// Tool dimensions are not all positive.
    #[error("diameter or length is either negative or zero for tool number '{0}'")]
    ToolNonPositive(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic = "failed to read file at 'notfound'"]
    fn file() {
        Config::from_file("notfound").unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn good() {
        let json = r#"
            {
              "units": "metric",
              "stock": {
                "x": 500,
                "y": 500,
                "z": 500
              },
              "zero_pos": {
                "x": 0,
                "y": 0,
                "z": 0
              },
              "start_pos": {
                "x": 250,
                "y": 250,
                "z": 750
              },
              "tools": [
                {
                  "number": 1,
                  "diameter": 5,
                  "length": 10
                },
                {
                  "number": 2,
                  "diameter": 5,
                  "length": 10
                }
              ],
              "default_tool": {
                "number": 0,
                "diameter": 10,
                "length": 20
              }
            }"#;

        let ret = Config::from_str(json).unwrap();

        // stock and tool sizes will always be positive regardless of the sign
        assert_eq!(
            ret,
            Config {
                units: Unit::Metric,
                stock: Point {
                    x: 500.0,
                    y: 500.0,
                    z: 500.0
                },
                zero_pos: Point {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                start_pos: Point {
                    x: 250.0,
                    y: 250.0,
                    z: 750.0
                },
                tools: vec![
                    ToolConfig {
                        number: 1,
                        diameter: 5.0,
                        length: 10.0,
                    },
                    ToolConfig {
                        number: 2,
                        diameter: 5.0,
                        length: 10.0,
                    }
                ],
                default_tool: ToolConfig {
                    number: 0,
                    diameter: 10.0,
                    length: 20.0
                }
            }
        );
    }

    #[test]
    #[should_panic = "unknown variant `invalid`, expected `metric` or `imperial`"]
    fn bad_units() {
        let json = r#"
            {
              "units": "invalid",
              "stock": {
                "x": 500,
                "y": 500,
                "z": 500
              },
              "zero_pos": {
                "x": 0,
                "y": 0,
                "z": 0
              },
              "start_pos": {
                "x": 250,
                "y": 250,
                "z": 750
              },
              "tools": [
                {
                  "number": 1,
                  "diameter": 5,
                  "length": 10
                },
                {
                  "number": 2,
                  "diameter": 5,
                  "length": 10
                }
              ],
              "default_tool": {
                "number": 0,
                "diameter": 10,
                "length": 20
              }
            }"#;

        Config::from_str(json).unwrap();
    }

    #[test]
    #[should_panic = "unknown field `excess`, expected one of `units`, `stock`, `zero_pos`, `start_pos`, `tools`, `default_tool`"]
    fn excess() {
        let json = r#"
            {
              "units": "metric",
              "stock": {
                "x": 500,
                "y": 500,
                "z": 500
              },
              "zero_pos": {
                "x": 0,
                "y": 0,
                "z": 0
              },
              "start_pos": {
                "x": 250,
                "y": 250,
                "z": 750
              },
              "tools": [
                {
                  "number": 1,
                  "diameter": 5,
                  "length": 10
                },
                {
                  "number": 2,
                  "diameter": 5,
                  "length": 10
                }
              ],
              "excess": "invalid"
            }"#;

        Config::from_str(json).unwrap();
    }

    #[test]
    #[should_panic = "a stock dimension is either negative or zero"]
    fn invalid_stock() {
        let json = r#"
            {
              "units": "metric",
              "stock": {
                "x": -500,
                "y": 500,
                "z": 500
              },
              "zero_pos": {
                "x": 0,
                "y": 0,
                "z": 0
              },
              "start_pos": {
                "x": 250,
                "y": 250,
                "z": 750
              },
              "tools": [],
              "default_tool": {
                "number": 0,
                "diameter": 10,
                "length": 20
              }
            }"#;

        Config::from_str(json).unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    #[should_panic = "diameter or length is either negative or zero for tool number '2'"]
    fn invalid_tool() {
        let json = r#"
            {
              "units": "metric",
              "stock": {
                "x": 500,
                "y": 500,
                "z": 500
              },
              "zero_pos": {
                "x": 0,
                "y": 0,
                "z": 0
              },
              "start_pos": {
                "x": 250,
                "y": 250,
                "z": 750
              },
              "tools": [
                {
                  "number": 2,
                  "diameter": -5,
                  "length": 10
                }
              ],
              "default_tool": {
                "number": 0,
                "diameter": 10,
                "length": 20
              }
            }"#;

        Config::from_str(json).unwrap_or_else(|e| panic!("{e}"));
    }
}
