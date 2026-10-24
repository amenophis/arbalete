//! The curves the sidebar slides along: how far along its way, from 0 to 1, at a fraction of its
//! duration, from 0 to 1.

/// How fast the floating sidebar's spring settles, and how fast it swings: it crosses its rest
/// position at 40 % of its duration, then goes about 1.6 % past it before settling.
const SPRING_DAMPING: f64 = 6.5;
const SPRING_FREQUENCY: f64 = 3.93;

/// Fast, then slowing down to a stop, without going past it.
pub fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4)
}

/// Fast, a little past the end, then back to it.
pub fn spring(t: f64) -> f64 {
    if t >= 1.0 {
        return 1.0;
    }
    let t = t.max(0.0);
    1.0 - (-SPRING_DAMPING * t).exp() * (SPRING_FREQUENCY * t).cos()
}

/// `t` of the way from `from` to `to`, to the nearest point.
pub fn lerp(from: i32, to: i32, t: f64) -> i32 {
    (f64::from(from) + f64::from(to - from) * t).round() as i32
}

/// `steps` + 1 evenly timed positions from `from` to `to` along `curve`, for the compositor to
/// slide through.
pub fn positions(curve: fn(f64) -> f64, from: f64, to: f64, steps: u32) -> Vec<f64> {
    (0..=steps)
        .map(|step| from + (to - from) * curve(f64::from(step) / f64::from(steps)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ease_out, lerp, positions, spring};

    fn samples(curve: fn(f64) -> f64) -> Vec<f64> {
        (0..=100).map(|i| curve(f64::from(i) / 100.0)).collect()
    }

    #[test]
    fn eases_out_from_start_to_end_without_overshoot() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        let values = samples(ease_out);
        assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(values.iter().all(|&value| value <= 1.0));
    }

    #[test]
    fn eases_out_strongly() {
        assert!(ease_out(0.25) > 0.65);
        assert!(ease_out(0.5) > 0.9);
    }

    #[test]
    fn the_spring_goes_a_little_past_the_end_and_settles_on_it() {
        assert_eq!(spring(0.0), 0.0);
        assert_eq!(spring(1.0), 1.0);
        assert!((spring(0.99) - 1.0).abs() < 0.005);
        let peak = samples(spring).into_iter().fold(0.0, f64::max);
        assert!(peak > 1.01 && peak < 1.025, "{peak}");
    }

    #[test]
    fn the_spring_does_most_of_its_way_first() {
        assert!(spring(0.2) > 0.6);
        assert!(spring(0.4) > 0.99);
    }

    #[test]
    fn interpolates_to_the_nearest_point() {
        assert_eq!(lerp(269, 9, 0.0), 269);
        assert_eq!(lerp(269, 9, 0.5), 139);
        assert_eq!(lerp(269, 9, 1.0), 9);
        assert_eq!(lerp(0, 3, 0.5), 2);
    }

    #[test]
    fn spreads_positions_along_a_curve() {
        assert_eq!(
            positions(|t| t, -260.0, 0.0, 4),
            [-260.0, -195.0, -130.0, -65.0, 0.0]
        );
        let slide = positions(spring, -260.0, 0.0, 30);
        assert_eq!((slide[0], slide[30]), (-260.0, 0.0));
        assert!(slide.iter().any(|&position| position > 2.0));
    }
}
