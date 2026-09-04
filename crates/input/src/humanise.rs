// SPDX-License-Identifier: MPL-2.0
use crate::Humanisation;

/// Jittered delays, so a sequence does not look like a metronome (ADR-0007).
///
/// Seeded and deterministic on purpose: the point is that consecutive delays
/// differ, and that property is only testable if the sequence can be replayed.
pub(crate) struct Jitter {
    state: u64,
}

impl Jitter {
    pub(crate) fn new(seed: u64) -> Self {
        Jitter {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub(crate) fn delay_ms(&mut self, humanisation: Humanisation) -> u64 {
        let (low, high) = if humanisation.min_delay_ms <= humanisation.max_delay_ms {
            (humanisation.min_delay_ms, humanisation.max_delay_ms)
        } else {
            (humanisation.max_delay_ms, humanisation.min_delay_ms)
        };
        low + self.next() % (high - low + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: u64, max: u64) -> Humanisation {
        Humanisation {
            min_delay_ms: min,
            max_delay_ms: max,
        }
    }

    #[test]
    fn every_delay_falls_inside_the_configured_range() {
        let mut jitter = Jitter::new(7);
        let bounds = range(40, 160);

        for _ in 0..500 {
            let delay = jitter.delay_ms(bounds);
            assert!((40..=160).contains(&delay), "got {delay}");
        }
    }

    #[test]
    fn consecutive_delays_differ() {
        let mut jitter = Jitter::new(7);
        let bounds = range(40, 160);

        let sample: Vec<u64> = (0..20).map(|_| jitter.delay_ms(bounds)).collect();
        let first = sample[0];

        assert!(
            sample.iter().any(|delay| *delay != first),
            "a constant delay is a metronome, which is the thing this exists to avoid"
        );
    }

    #[test]
    fn a_fixed_range_yields_that_value() {
        let mut jitter = Jitter::new(7);

        assert_eq!(jitter.delay_ms(range(100, 100)), 100);
    }

    #[test]
    fn an_inverted_range_is_read_the_right_way_round() {
        let mut jitter = Jitter::new(7);

        for _ in 0..100 {
            let delay = jitter.delay_ms(range(160, 40));
            assert!(
                (40..=160).contains(&delay),
                "a reversed range must not panic or return nonsense, got {delay}"
            );
        }
    }

    #[test]
    fn a_zero_seed_still_produces_a_sequence() {
        let mut jitter = Jitter::new(0);
        let bounds = range(0, 1000);

        let sample: Vec<u64> = (0..10).map(|_| jitter.delay_ms(bounds)).collect();

        assert!(
            sample.iter().any(|delay| *delay != sample[0]),
            "seeding with zero must not collapse the generator"
        );
    }
}
