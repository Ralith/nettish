use std::time::Duration;

/// Compute the amount of time to advance a simulation after `real_time` has passed, given
/// `buffer_remaining` simulation time until data is exhausted
///
/// If `buffer_remaining` is between `min_latency` and `min_latency + hysteresis`, `real_time` is
/// returned exactly. Otherwise, the returned time is smoothly scaled to gradually bring it back
/// into that window without discontinuities. Never returns a value greater than `buffer_remaining`.
///
/// - `real_time` - Amount of wall-clock time passed.
/// - `buffer_remaining` - Amount the simulation can progress without running out of data. Grows
///   as data is received from the server, and shrinks as simulation time progresses.
/// - `min_latency` - When `buffer_remaining` is below this, simulation time slows. Smaller values
///   reduce latency under ideal conditions at the cost of requiring more abrupt corrections for
///   network latency jitter and when the server falls behind.
/// - `hysteresis` - [`Duration`] of the interval within which time flows normally. Should match the
///   expected interval between updates received from the server to keep the flow of time uniform
///   under ideal conditions.
pub fn throttle(
    real_time: Duration,
    buffer_remaining: Duration,
    min_latency: Duration,
    hysteresis: Duration,
) -> Duration {
    let scaled = if let Some(error) = min_latency.checked_sub(buffer_remaining) {
        // We're about to run out of data; slow down
        #[allow(clippy::manual_clamp)] // NaN handling in case `min_latency` is too close to zero
        let scale = 1.0 - f32::min(1.0, f32::max(0.0, error.div_duration_f32(min_latency)));
        real_time.mul_f32(scale)
    } else if let Some(error) = buffer_remaining.checked_sub(min_latency + hysteresis) {
        // We've fallen too far behind; speed up. 1 second behind = 2x speed
        let scale = error.as_secs_f32();
        // We know we're at least `error` behind where we should be, but not necessarily any
        // further. If we overshoot we'll underrun in the future.
        real_time + Ord::min(real_time.mul_f32(scale), error)
    } else {
        real_time
    };
    // If `real_time` is large, we might overshoot the entire buffer.
    Ord::min(scaled, buffer_remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        assert_eq!(
            throttle(
                Duration::from_millis(10),
                Duration::from_millis(100),
                Duration::from_millis(50),
                Duration::from_millis(200),
            ),
            Duration::from_millis(10),
            "time proceeds uniformly within the hysteresis region"
        );
        assert!(
            throttle(
                Duration::from_millis(10),
                Duration::from_millis(40),
                Duration::from_millis(50),
                Duration::from_millis(200),
            ) < Duration::from_millis(10),
            "slow down to preserve jitter buffer"
        );
        assert!(
            throttle(
                Duration::from_millis(10),
                Duration::from_millis(1000),
                Duration::from_millis(50),
                Duration::from_millis(200),
            ) > Duration::from_millis(10),
            "speed up when far behind"
        );
    }

    #[test]
    fn large_step() {
        assert_eq!(
            throttle(
                Duration::from_millis(1_000_000),
                Duration::from_millis(40),
                Duration::from_millis(50),
                Duration::from_millis(200),
            ),
            Duration::from_millis(40),
        );
        assert_eq!(
            throttle(
                Duration::from_millis(1_000_000),
                Duration::from_millis(1000),
                Duration::from_millis(50),
                Duration::from_millis(200),
            ),
            Duration::from_millis(1000),
        );
    }

    #[test]
    fn sim_ideal() {
        const MIN_LATENCY: Duration = Duration::from_millis(10);
        const STEP_INTERVAL: Duration = Duration::from_millis(33);
        const FRAME_INTERVAL: Duration = Duration::from_millis(7);
        let mut buffer_remaining = Duration::ZERO;
        let mut time_in_step = Duration::ZERO;
        for _ in 0..1_000 {
            let sim_time = throttle(FRAME_INTERVAL, buffer_remaining, MIN_LATENCY, STEP_INTERVAL);
            dbg!(sim_time);
            // If we start ahead, we should never need to catch up
            assert!(sim_time <= FRAME_INTERVAL);
            // Guaranteed not to overrun
            assert!(sim_time <= buffer_remaining);
            buffer_remaining -= sim_time;
            // Simulate regular updates from server
            time_in_step += FRAME_INTERVAL;
            if time_in_step > STEP_INTERVAL {
                time_in_step -= STEP_INTERVAL;
                buffer_remaining += STEP_INTERVAL;
            }
        }
    }
}
