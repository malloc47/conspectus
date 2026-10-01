//! Cursor arithmetic for lists navigated by signed steps.

/// `index` moved `delta` places through a list of `len` items, wrapping
/// past either end. Returns 0 for an empty list.
pub(crate) fn wrap_step(index: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let step = usize::try_from(delta.unsigned_abs()).map_or(0, |step| step % len);
    let index = index % len;
    if delta >= 0 {
        (index + step) % len
    } else {
        (index + len - step) % len
    }
}

/// `index` moved `delta` places through a list of `len` items, stopping
/// at either end. Returns 0 for an empty list.
pub(crate) fn clamp_step(index: usize, len: usize, delta: i32) -> usize {
    let step = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
    let moved = if delta >= 0 {
        index.saturating_add(step)
    } else {
        index.saturating_sub(step)
    };
    moved.min(len.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_step_wraps_both_ways() {
        assert_eq!(wrap_step(0, 3, 1), 1);
        assert_eq!(wrap_step(2, 3, 1), 0);
        assert_eq!(wrap_step(0, 3, -1), 2);
        assert_eq!(wrap_step(1, 3, -4), 0);
        assert_eq!(wrap_step(1, 3, 7), 2);
        assert_eq!(wrap_step(5, 0, 1), 0);
    }

    #[test]
    fn clamp_step_stops_at_the_ends() {
        assert_eq!(clamp_step(0, 3, -1), 0);
        assert_eq!(clamp_step(1, 3, 1), 2);
        assert_eq!(clamp_step(2, 3, 10), 2);
        assert_eq!(clamp_step(2, 3, -10), 0);
        assert_eq!(clamp_step(0, 0, 1), 0);
    }
}
