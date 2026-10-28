//! Moving a cursor through a list by signed steps.

/// `index` moved `delta` places through a list of `len` items, wrapping
/// past either end.
///
/// For short choice lists (menus, pickers, checklists, palettes), where
/// the far end is a step away (house rule 2). A step longer than the list
/// goes round as many times as it takes. An index past the end is first
/// taken back into the list. Returns 0 for an empty list.
///
/// ```
/// use tuikit::scroll::wrap_step;
///
/// assert_eq!(wrap_step(2, 3, 1), 0);
/// assert_eq!(wrap_step(0, 3, -1), 2);
/// ```
pub fn wrap_step(index: usize, len: usize, delta: i32) -> usize {
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
/// at either end.
///
/// For scrollable content (trees, property lists, pagers, logs), where
/// running past the end should not jump to the start (house rule 2). A
/// page key passes the window's height as `delta`. Returns 0 for an empty
/// list.
///
/// ```
/// use tuikit::scroll::clamp_step;
///
/// assert_eq!(clamp_step(2, 3, 10), 2);
/// assert_eq!(clamp_step(2, 3, -10), 0);
/// ```
pub fn clamp_step(index: usize, len: usize, delta: i32) -> usize {
    let step = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
    let moved = if delta >= 0 {
        index.saturating_add(step)
    } else {
        index.saturating_sub(step)
    };
    moved.min(len.saturating_sub(1))
}
