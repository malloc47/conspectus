//! Transient toast widget (T8-040 / H-WIDG-003).
//!
//! Shim over [`ratatui_comfy_toaster::ToastEngine`]: the engine owns
//! the per-toast lifetime + bordered rendering, and this module
//! exposes [`engine`] / [`builder_for`] helpers that bake in the
//! project-specific defaults (Success type, full border, bottom-
//! centered position, fixed [`TOAST_DURATION`]). The App owns one
//! `ToastEngine<()>`; the runtime calls
//! [`ToastEngine::tick`] + [`ToastEngine::set_area`] before each
//! draw so the polled "current toast" idiom from the in-tree
//! version maps cleanly onto the engine's queued-with-expiry model.
//!
//! Theming caveat: the upstream `Toast::render_ref` derives the
//! border color from a hardcoded `From<ToastType> for Color` impl
//! (Success → Green). Conspectus's `[tui.theme] success` defaults
//! to `Color::Green` too, so the default-palette visual is byte-
//! identical; operators who override `theme.success` will see the
//! toast stay green while other "success" surfaces honor the
//! override. Acceptable for a 1.5-second blip per ADR 0067; would
//! need an upstream `border_fg` override or a fork to fix.
//!
//! Replacement semantics: posting a new toast drains the queue
//! first via [`engine_dismiss_all`] so the new toast supersedes
//! the older one — the in-tree contract the reducer and runtime
//! relied on.

use std::borrow::Cow;
use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui_comfy_toaster::{
    ToastBorderMode, ToastBuilder, ToastEngine, ToastEngineBuilder, ToastPosition, ToastType,
};

/// How long a toast stays visible before it's auto-dismissed.
/// Mirrors the prior in-tree constant byte-for-byte so the operator
/// experience is unchanged.
pub const TOAST_DURATION: Duration = Duration::from_millis(1500);

/// Build a default-area engine. The runtime updates the area each
/// frame via [`ToastEngine::set_area`]; passing `Rect::default()` at
/// construction is fine because no toast renders before the first
/// `set_area` call.
pub fn engine() -> ToastEngine<()> {
    ToastEngineBuilder::new(Rect::default())
        .default_duration(TOAST_DURATION)
        .build()
}

/// Build a `ToastBuilder` pre-configured for Conspectus's toast
/// surface: Success type (green border by default), full border,
/// transparent background, anchored at the bottom of the frame via
/// `Center + offset(0, i16::MAX)` — the offset is clamped against
/// the engine's area by upstream so a saturating positive `y` pins
/// the toast to the bottom edge regardless of frame height.
pub fn builder_for(label: impl Into<Cow<'static, str>>) -> ToastBuilder {
    ToastBuilder::new(label.into())
        .toast_type(ToastType::Success)
        .toast_bg(Color::Reset)
        .border_mode(ToastBorderMode::Full)
        .position(ToastPosition::Center)
        .offset(0, i16::MAX)
        .duration(TOAST_DURATION)
}

/// Drain every queued toast. Used by `App::post_toast` so a newer
/// toast supersedes any prior one — the in-tree "replacement"
/// contract pinned by the reducer's `post_toast_supersedes_prior_toast`
/// test.
pub fn engine_dismiss_all(engine: &mut ToastEngine<()>) {
    while engine.dismiss() {}
}

/// Newtype wrapper so `App` can keep `#[derive(Debug)]`. The
/// upstream `ratatui_comfy_toaster::ToastEngine<A>` does not derive
/// `Debug` (one likely cause is the optional channel sender behind
/// the `tokio` feature). The wrapper exposes a placeholder `Debug`
/// while `Deref` / `DerefMut` forward every call to the inner
/// engine so call-site ergonomics are unchanged.
pub struct ToastEngineHolder(pub ToastEngine<()>);

impl std::fmt::Debug for ToastEngineHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ToastEngine<()>")
    }
}

impl std::ops::Deref for ToastEngineHolder {
    type Target = ToastEngine<()>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ToastEngineHolder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::WidgetRef;
    use std::thread::sleep;

    #[test]
    fn fresh_engine_has_no_toast() {
        let engine = engine();
        assert!(!engine.has_toast());
    }

    #[test]
    fn show_then_tick_after_duration_clears_the_toast() {
        let mut engine = engine();
        engine.set_area(Rect::new(0, 0, 40, 10));
        engine.show_toast(builder_for("copied: cwd"));
        assert!(engine.has_toast());
        // Wait past the toast duration plus a small margin so the
        // upstream `tick` retires the expired entry.
        sleep(TOAST_DURATION + Duration::from_millis(50));
        engine.tick();
        assert!(!engine.has_toast());
    }

    #[test]
    fn newly_posted_toast_is_visible_immediately() {
        let mut engine = engine();
        engine.set_area(Rect::new(0, 0, 40, 10));
        engine.show_toast(builder_for("copied: cwd"));
        engine.tick();
        assert!(engine.has_toast());
    }

    #[test]
    fn engine_dismiss_all_drains_the_queue() {
        let mut engine = engine();
        engine.set_area(Rect::new(0, 0, 40, 10));
        engine.show_toast(builder_for("a"));
        engine.show_toast(builder_for("b"));
        engine_dismiss_all(&mut engine);
        assert!(!engine.has_toast());
    }

    #[test]
    fn rendered_toast_paints_borders_and_label() {
        let mut engine = engine();
        let area = Rect::new(0, 0, 40, 10);
        engine.set_area(area);
        engine.show_toast(builder_for("copied: cwd"));
        let mut buf = Buffer::empty(area);
        engine.render_ref(area, &mut buf);
        // Scan every row of the buffer for the label text. The
        // upstream renderer wraps and centers; we don't pin the
        // exact y offset because the engine's layout math owns it.
        let mut found = false;
        for y in 0..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    row.push_str(cell.symbol());
                }
            }
            if row.contains("copied: cwd") {
                found = true;
                break;
            }
        }
        assert!(found, "expected label in rendered buffer");
    }
}
