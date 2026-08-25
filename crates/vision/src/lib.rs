// SPDX-License-Identifier: MPL-2.0
//! Perception (ADR-0006).
//!
//! The hard problem here is **not** matching, it is **anchoring**. Resolution,
//! DPI, UI language and game patches all break raw pixel coordinates at once.
//! Every region is therefore expressed relative to the window client area, and
//! optionally re-registered against a stable anchor before matching.

use idlewarden_capture::Frame;
use idlewarden_plugin_api::{Confidence, SignalId, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("anchor `{0}` not found; the layout could not be registered")]
    AnchorLost(String),
    #[error("region {0:?} falls outside the window")]
    RegionOutOfBounds(Roi),
    #[error("ocr failed: {0}")]
    Ocr(String),
}

/// A region of interest in window-relative coordinates (`0.0..=1.0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Roi {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Roi {
    pub fn is_within_unit_square(&self) -> bool {
        self.x >= 0.0
            && self.y >= 0.0
            && self.w > 0.0
            && self.h > 0.0
            && self.x + self.w <= 1.0
            && self.y + self.h <= 1.0
    }

    /// Convert to physical pixels for a given frame.
    pub fn to_pixels(&self, frame: &Frame) -> (u32, u32, u32, u32) {
        let (fw, fh) = (frame.size.width as f64, frame.size.height as f64);
        (
            (self.x * fw) as u32,
            (self.y * fh) as u32,
            (self.w * fw) as u32,
            (self.h * fh) as u32,
        )
    }

    /// Shift by the offset produced by anchor registration.
    pub fn translated(&self, dx: f64, dy: f64) -> Roi {
        Roi {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

/// A visually stable element used to re-register the layout before matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anchor {
    pub name: String,
    pub search_area: Roi,
    /// Template asset name, resolved inside the plugin package.
    pub template: String,
    /// Below this, we consider the anchor lost rather than mis-placed.
    pub min_score: f64,
}

/// How a signal is extracted from a frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Extractor {
    /// Normalised cross-correlation against a template asset.
    TemplateMatch {
        roi: Roi,
        template: String,
        min_score: f64,
    },
    /// Read text, then parse it as the declared value type.
    Ocr { roi: Roi },
    /// Presence of a colour within tolerance — cheapest and surprisingly
    /// effective for idle-game state (button enabled, resource full).
    ColorProbe {
        roi: Roi,
        rgb: [u8; 3],
        tolerance: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRule {
    pub id: SignalId,
    pub extractor: Extractor,
}

/// The result of one extraction, before it becomes a `Signal`.
#[derive(Debug, Clone)]
pub struct Extracted {
    pub id: SignalId,
    pub value: Value,
    pub confidence: Confidence,
}

pub trait Perceiver: Send {
    /// Re-register the layout, then extract every rule. Implementations must
    /// return a *low confidence* rather than an error when a single rule is
    /// merely uncertain: errors are for structural failures.
    fn perceive(&mut self, frame: &Frame) -> Result<Vec<Extracted>, VisionError>;
}
