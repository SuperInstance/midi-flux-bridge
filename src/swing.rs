//! Swing timing computation and groove templates.
//!
//! Provides off-beat delay ratio calculation and predefined groove templates:
//! straight, triplet, dotted, and custom swing patterns.

use serde::{Deserialize, Serialize};

/// A groove template defining swing timing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrooveTemplate {
    pub name: String,
    /// Ratios for each subdivision. 1.0 = straight, >1.0 = delayed off-beat.
    pub ratios: Vec<f64>,
}

impl GrooveTemplate {
    /// Create a new groove template.
    pub fn new(name: impl Into<String>, ratios: Vec<f64>) -> Self {
        Self {
            name: name.into(),
            ratios,
        }
    }

    /// Apply this groove to a base interval, returning beat offsets.
    pub fn apply(&self, base_interval_ms: f64, beats: usize) -> Vec<f64> {
        let mut result = Vec::new();
        let mut t = 0.0;
        for i in 0..beats {
            result.push(t);
            let ratio = self.ratios[i % self.ratios.len()];
            t += base_interval_ms * ratio;
        }
        result
    }

    /// The average swing ratio.
    pub fn average_ratio(&self) -> f64 {
        if self.ratios.is_empty() {
            return 1.0;
        }
        self.ratios.iter().sum::<f64>() / self.ratios.len() as f64
    }
}

/// Predefined straight groove: all beats equal.
pub fn straight_template() -> GrooveTemplate {
    GrooveTemplate::new("straight", vec![1.0])
}

/// Triplet groove: 2:1 ratio (long-short).
pub fn triplet_template() -> GrooveTemplate {
    GrooveTemplate::new("triplet", vec![2.0 / 3.0, 1.0 / 3.0])
}

/// Dotted groove: dotted-eighth + sixteenth pattern (3:1).
pub fn dotted_template() -> GrooveTemplate {
    GrooveTemplate::new("dotted", vec![0.75, 0.25])
}

/// Custom groove with specified swing ratio.
pub fn custom_template(swing_ratio: f64) -> GrooveTemplate {
    GrooveTemplate::new("custom", vec![1.0 - swing_ratio / 2.0, swing_ratio / 2.0])
}

/// Compute swing-delayed timestamps for a sequence of beats.
///
/// `swing` is a value 0.0–1.0 where 0.0 = straight and 1.0 = maximum swing.
/// Off-beats (odd indices) are delayed by `swing * beat_interval_ms`.
pub fn apply_swing(beat_interval_ms: f64, swing: f64, count: usize, offset_ms: f64) -> Vec<f64> {
    let mut beats = Vec::new();
    let mut t = offset_ms;
    for i in 0..count {
        beats.push(t);
        let swing_delay = if i % 2 == 1 {
            swing * beat_interval_ms
        } else {
            0.0
        };
        t += beat_interval_ms + swing_delay;
    }
    beats
}

/// Compute the effective swing ratio between on-beat and off-beat durations.
pub fn swing_ratio(on_beat_ms: f64, off_beat_ms: f64) -> f64 {
    if on_beat_ms == 0.0 {
        return 0.0;
    }
    off_beat_ms / on_beat_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_straight_template_equal_beats() {
        let tmpl = straight_template();
        let beats = tmpl.apply(500.0, 4);
        assert_eq!(beats, vec![0.0, 500.0, 1000.0, 1500.0]);
    }

    #[test]
    fn test_triplet_template_2_to_1_ratio() {
        let tmpl = triplet_template();
        let beats = tmpl.apply(600.0, 4);
        // Beat 0: 0, Beat 1: 400, Beat 2: 600, Beat 3: 1000
        assert!((beats[0] - 0.0).abs() < 0.01);
        assert!((beats[1] - 400.0).abs() < 0.01);
        assert!((beats[2] - 600.0).abs() < 0.01);
        assert!((beats[3] - 1000.0).abs() < 0.01);
    }

    #[test]
    fn test_dotted_template_3_to_1() {
        let tmpl = dotted_template();
        let beats = tmpl.apply(400.0, 4);
        assert!((beats[0] - 0.0).abs() < 0.01);
        assert!((beats[1] - 300.0).abs() < 0.01);
        assert!((beats[2] - 400.0).abs() < 0.01);
        assert!((beats[3] - 700.0).abs() < 0.01);
    }

    #[test]
    fn test_custom_template() {
        let tmpl = custom_template(0.5);
        assert_eq!(tmpl.ratios.len(), 2);
        let beats = tmpl.apply(400.0, 4);
        assert!((beats[0] - 0.0).abs() < 0.01);
        // ratio 0.75 * 400 = 300
        assert!((beats[1] - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_swing_no_swing() {
        let beats = apply_swing(500.0, 0.0, 4, 0.0);
        assert_eq!(beats, vec![0.0, 500.0, 1000.0, 1500.0]);
    }

    #[test]
    fn test_apply_swing_with_swing() {
        let beats = apply_swing(500.0, 0.5, 4, 0.0);
        // Off-beat delay = 0.5 * 500 = 250
        // 0, 500, 1000+250=1250, 1750
        assert!((beats[0] - 0.0).abs() < 0.01);
        assert!((beats[1] - 500.0).abs() < 0.01);
        assert!((beats[2] - 1250.0).abs() < 0.01);
        assert!((beats[3] - 1750.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_swing_with_offset() {
        let beats = apply_swing(500.0, 0.0, 3, 100.0);
        assert_eq!(beats, vec![100.0, 600.0, 1100.0]);
    }

    #[test]
    fn test_swing_ratio() {
        assert!((swing_ratio(500.0, 500.0) - 1.0).abs() < 0.01);
        assert!((swing_ratio(500.0, 250.0) - 0.5).abs() < 0.01);
        assert!((swing_ratio(400.0, 800.0) - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_groove_average_ratio() {
        let tmpl = triplet_template();
        let avg = tmpl.average_ratio();
        assert!((avg - (2.0/3.0 + 1.0/3.0) / 2.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_ratios_average() {
        let tmpl = GrooveTemplate::new("empty", vec![]);
        assert_eq!(tmpl.average_ratio(), 1.0);
    }
}
