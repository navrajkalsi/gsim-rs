//! # GSim Configuration
//!
//! Configuration file parser.
//! Reads and parses **JSON** config file at a provided file path.
//!
//! ## Example
//! The following is an example of a valid config file:
//! ```text
//! {
//!  "setup": "milling",
//!  "units": "metric",
//!  "stock": {
//!    "shape": "cuboid",
//!    "dimensions": {
//!      "x": 500,
//!      "y": 500,
//!      "z": 500
//!    }
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
//!  ]
//! }
//! ```
//! - Sets up a `milling` simulation.
//! - Treats every dimension in `metric` system.
//! - Creates a `cuboid` shaped stock, with each side measuring `500mm`.
//! - Does not offset reference point of the stock, and sets it as the `zero_pos`. Here, for a
//!   `cuboid` stock, the reference point is the **left-bottom-near** point.
//! - Starts the simulation at `start_pos`, which is offset from the reference point. Here, it
//!   will start at middle of **X** and **Y** of the stock and **250mm** above the stock.
//! - Creates two tools(numbered `1` & `2`), each with `diameter` `5mm` and `length` `10mm`.
//!
//! ## Restrictions
//! - Any **excess elements** will be rejected.
//! - `setup` can only have two possible values: `milling` or `turning`.
//! - `units` can only have two possible values: `imperial` or `metric`.
//! - Stock `shape` can only have two possible values: `cuboid` or `cylinder`.
//! - Every stock dimension **must** be positive and non-zero.
//! - Each tool `diameter` and `length` **must** be positive and non-zero.

use std::str::FromStr;

use crate::{FLOAT_VARIANCE, points::Point};
use serde::Deserialize;

/// Program configuration at start.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Machining setup for the entire program.
    pub setup: Setup,
    /// Unit system applied to all dimensional values (e.g. `stock_size`, `tool_length`).
    /// [`Machine`](crate::machine) will also be configured with this system.
    pub units: Unit,
    /// Stock body description.
    pub stock: Body,
    /// Work offset zero position.
    /// This is relative to a **stock reference point**.
    /// Check [`Stock`] for details on reference point.
    pub zero_pos: Point,
    /// Start position at the beginning of the program.
    /// This is relative to **stock reference point**.
    /// Check [`Stock`] for details on reference point.
    pub start_pos: Point,
    /// Collection of tool configurations to be used during G-code execution.
    pub tools: Vec<ToolConfig>,
}

/// Available types of machining setups.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Setup {
    Milling,
    Turning,
}

/// Possible unit standards for dimensional values.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Metric,
    Imperial,
}

/// Description of a stock, irrespective of the machining setup.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "shape", content = "dimensions", rename_all = "lowercase")]
pub enum Body {
    /// A solid box.
    /// Reference point is at `0.0` for each axis (**bottom-left-near**).
    Cuboid { x: f32, y: f32, z: f32 },

    /// A solid cylinder.
    /// Reference point is also at `0.0` for each axis (**center of base-face**).
    Cylinder {
        /// Axis along which the curved face should be laid.
        axis: Axis,
        /// Diameter of the cylinder.
        diameter: f32,
        /// Distance between both circular faces of the cylinder.
        length: f32,
    },
}

/// Axis choices for a 3 axis setup.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Tool configuration.
#[derive(Clone, Debug, Deserialize, PartialEq)]
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
        match &ret.stock {
            Body::Cuboid { x, y, z } => {
                if let Setup::Turning = ret.setup {
                    return Err(ConfigError::TurningStock); // unusual turning stock
                }

                if *x < FLOAT_VARIANCE || *y < FLOAT_VARIANCE || *z < FLOAT_VARIANCE {
                    return Err(ConfigError::StockNonPositive);
                }
            }

            Body::Cylinder {
                axis,
                diameter,
                length,
            } => {
                if let Setup::Turning = ret.setup
                    && !matches!(axis, Axis::Z)
                {
                    return Err(ConfigError::TurningStock); // unusual turning stock setup
                }

                if *diameter < FLOAT_VARIANCE || *length < FLOAT_VARIANCE {
                    return Err(ConfigError::StockNonPositive);
                }
            }
        };

        for tool in &ret.tools {
            if tool.diameter < FLOAT_VARIANCE || tool.length < FLOAT_VARIANCE {
                return Err(ConfigError::ToolNonPositive(tool.number));
            }
        }

        Ok(ret)
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
    /// Abnormal turning stock setup.
    #[error("the stock description/setup is abnormal for a turning setup")]
    TurningStock,
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
              "setup": "milling",
              "units": "metric",
              "stock": {
                "shape": "cuboid",
                "dimensions": {
                  "x": 500,
                  "y": 500,
                  "z": 500
                }
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
              ]
            }"#;

        let ret = Config::from_str(json).unwrap();

        // stock and tool sizes will always be positive regardless of the sign
        assert_eq!(
            ret,
            Config {
                setup: Setup::Milling,
                units: Unit::Metric,
                stock: Body::Cuboid {
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
                ]
            }
        );
    }

    #[test]
    #[should_panic = "unknown variant `invalid`, expected `metric` or `imperial`"]
    fn bad_units() {
        let json = r#"
            {
              "setup": "milling",
              "units": "invalid",
              "stock": {
                "shape": "cuboid",
                "dimensions": {
                  "x": 500,
                  "y": 500,
                  "z": 500
                }
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
              ]
            }"#;

        Config::from_str(json).unwrap();
    }

    #[test]
    #[should_panic = "unknown field `excess`, expected one of `setup`, `units`, `stock`, `zero_pos`, `start_pos`, `tools`"]
    fn excess() {
        let json = r#"
            {
              "setup": "milling",
              "units": "metric",
              "stock": {
                "shape": "cuboid",
                "dimensions": {
                  "x": 500,
                  "y": 500,
                  "z": 500
                }
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
              "setup": "milling",
              "units": "metric",
              "stock": {
                "shape": "cuboid",
                "dimensions": {
                  "x": -500,
                  "y": 500,
                  "z": 500
                }
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
              "tools": []
            }"#;

        Config::from_str(json).unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    #[should_panic = "diameter or length is either negative or zero for tool number '2'"]
    fn invalid_tool() {
        let json = r#"
            {
              "setup": "milling",
              "units": "metric",
              "stock": {
                "shape": "cuboid",
                "dimensions": {
                  "x": 500,
                  "y": 500,
                  "z": 500
                }
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
              ]
            }"#;

        Config::from_str(json).unwrap_or_else(|e| panic!("{e}"));
    }
}
