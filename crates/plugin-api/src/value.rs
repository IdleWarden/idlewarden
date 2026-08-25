// SPDX-License-Identifier: Apache-2.0
//! Dynamically-typed signal values.
//!
//! The Core must never know the shape of any particular game's state
//! (ADR-0002). A plugin declares a *schema* of signals in its manifest; at
//! runtime it emits [`Value`]s that the Core validates against that schema and
//! that the UI renders generically.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    /// A ratio in `0.0..=1.0`, e.g. a health or progress bar.
    Ratio(f64),
    Text(String),
    /// A point in *window-relative* coordinates (ADR-0003): never screen pixels.
    Point {
        x: f64,
        y: f64,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
    /// An opaque identifier, e.g. the id of the UI screen currently shown.
    Enum(String),
}

impl Value {
    /// The name of the variant, used for schema validation error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Ratio(_) => "ratio",
            Value::Text(_) => "text",
            Value::Point { .. } => "point",
            Value::Rect { .. } => "rect",
            Value::Enum(_) => "enum",
        }
    }

    /// `Ratio` values are the only ones with an enforced range.
    pub fn is_well_formed(&self) -> bool {
        match self {
            Value::Ratio(r) => (0.0..=1.0).contains(r),
            Value::Float(f) => f.is_finite(),
            Value::Point { x, y } => x.is_finite() && y.is_finite(),
            Value::Rect { x, y, w, h } => {
                x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()
            }
            _ => true,
        }
    }
}
