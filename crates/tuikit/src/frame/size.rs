//! Sizing policies: how large a modal is on a screen of any size.

use ratatui::layout::Rect;

/// How much of one direction of the screen a modal takes.
///
/// The modal aims for a `share` of the screen, takes no more than `cap`
/// cells, and keeps `margin` cells clear on each side; then it grows to
/// at least `min` cells, and never past the screen. In that order: the
/// margin gives way to the minimum on a small screen, and the screen
/// bounds everything. [`Extent::cells`] applies the rule.
///
/// ```
/// use tuikit::frame::Extent;
///
/// let width = Extent::new(64, 40, 2);
/// assert_eq!(width.cells(200, u16::MAX), 64, "the cap on a large screen");
/// assert_eq!(width.cells(60, u16::MAX), 56, "two cells clear on each side");
/// assert_eq!(width.cells(42, u16::MAX), 40, "the minimum beats the margin");
/// assert_eq!(width.cells(30, u16::MAX), 30, "the screen bounds the minimum");
///
/// let share = Extent::new(200, 60, 2).share(85);
/// assert_eq!(share.cells(100, u16::MAX), 85);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Extent {
    /// The share of the screen the modal aims for, in percent: 100 aims
    /// for all of it. Values outside 1 to 100 are read as the nearest end.
    pub share: u16,
    /// The most cells the modal takes, however large the screen.
    pub cap: u16,
    /// The fewest cells the modal takes while the screen has them.
    pub min: u16,
    /// The cells kept clear on each side while the screen is large
    /// enough that the minimum still fits.
    pub margin: u16,
}

impl Extent {
    /// An extent that aims for the whole screen: at most `cap` cells, at
    /// least `min`, keeping `margin` cells clear on each side.
    pub const fn new(cap: u16, min: u16, margin: u16) -> Self {
        Self {
            share: 100,
            cap,
            min,
            margin,
        }
    }

    /// The same extent aiming for `percent` of the screen instead of all
    /// of it.
    #[must_use]
    pub const fn share(self, percent: u16) -> Self {
        Self {
            share: percent,
            ..self
        }
    }

    /// The cells the extent takes of a screen `screen` cells long, for
    /// content that asks for `wanted` cells (`u16::MAX` asks for as many
    /// as the extent allows).
    pub fn cells(self, screen: u16, wanted: u16) -> u16 {
        let room = screen.saturating_sub(self.margin.saturating_mul(2));
        let share = u32::from(self.share.clamp(1, 100));
        let aim = u16::try_from(u32::from(screen) * share / 100).unwrap_or(screen);
        wanted
            .min(self.cap)
            .min(aim)
            .min(room)
            .max(self.min)
            .min(screen)
    }
}

/// Where a modal's height comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fit {
    /// As tall as the body's rows and the frame around them, within the
    /// extent. A body taller than that scrolls.
    Content,
    /// As tall as the extent allows, whatever the body holds, so the
    /// frame keeps its size while the body changes (a list being
    /// filtered, a page being scrolled).
    Fill,
}

/// A sizing policy: how wide and how tall a modal is on a screen of any
/// size. The width always takes what its extent allows; the height
/// follows [`Fit`].
///
/// The named policies are the framework's sizes, smallest to largest.
/// Each keeps two columns and one row clear on each side of the screen
/// while it can, so the screen's edges and its first and last lines (a
/// header and a status line) stay in view around a modal:
///
/// | Policy | Width | Height | For |
/// | --- | --- | --- | --- |
/// | [`COMPACT`](Self::COMPACT) | up to 60, at least 20 | the body's | a one-line prompt, a short checklist |
/// | [`MENU`](Self::MENU) | up to 64, at least 40 | the body's | a list of choices, a settings list |
/// | [`FORM`](Self::FORM) | up to 80, at least 44 | the body's | labeled fields |
/// | [`WIDE`](Self::WIDE) | up to 100, at least 40 | all, at least 10 | reading: help, a log, a long value; a long list |
/// | [`PALETTE`](Self::PALETTE) | 85% of the screen, 60 to 200 | 80%, 12 to 50 | a search palette |
///
/// ```
/// use ratatui::layout::Rect;
/// use tuikit::frame::ModalSize;
///
/// let screen = Rect::new(0, 0, 100, 30);
/// // A prompt of one row: the row and its border.
/// assert_eq!(ModalSize::COMPACT.rect(screen, 3), Rect::new(20, 14, 60, 3));
/// // A reader fills the screen but its margins, whatever it holds.
/// assert_eq!(ModalSize::WIDE.rect(screen, 3), Rect::new(2, 1, 96, 28));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModalSize {
    /// How wide the modal is.
    pub width: Extent,
    /// How tall the modal may be.
    pub height: Extent,
    /// Whether the height follows the body or fills the extent.
    pub fit: Fit,
}

/// The columns kept clear on each side of the screen.
const SIDE_MARGIN: u16 = 2;
/// The rows kept clear above and below.
const END_MARGIN: u16 = 1;
/// The fewest rows a modal that fits its body takes: the border and one
/// row.
const MIN_ROWS: u16 = 3;

impl ModalSize {
    /// A one-line prompt or a short checklist: up to 60 columns, at least
    /// 20, as tall as its rows.
    pub const COMPACT: Self = Self::content(60, 20);
    /// A list of choices or settings: up to 64 columns, at least 40, as
    /// tall as its rows.
    pub const MENU: Self = Self::content(64, 40);
    /// Labeled fields: up to 80 columns, at least 44, as tall as its
    /// rows.
    pub const FORM: Self = Self::content(80, 44);
    /// Reading (help, a log, a long value) and long lists: up to 100
    /// columns, at least 40, and all the rows the screen has but the
    /// margins, at least 10.
    pub const WIDE: Self = Self {
        width: Extent::new(100, 40, SIDE_MARGIN),
        height: Extent::new(u16::MAX, 10, END_MARGIN),
        fit: Fit::Fill,
    };
    /// A search palette: 85% of the screen's width (60 to 200 columns)
    /// and 80% of its height (12 to 50 rows), so a large screen shows
    /// more results.
    pub const PALETTE: Self = Self {
        width: Extent::new(200, 60, SIDE_MARGIN).share(85),
        height: Extent::new(50, 12, END_MARGIN).share(80),
        fit: Fit::Fill,
    };

    /// A policy whose height follows the body: up to `cap` columns, at
    /// least `min`.
    const fn content(cap: u16, min: u16) -> Self {
        Self {
            width: Extent::new(cap, min, SIDE_MARGIN),
            height: Extent::new(u16::MAX, MIN_ROWS, END_MARGIN),
            fit: Fit::Content,
        }
    }

    /// The modal's width on a screen `screen` columns wide.
    pub fn width(self, screen: u16) -> u16 {
        self.width.cells(screen, u16::MAX)
    }

    /// The modal's height on a screen `screen` rows tall, for a body that
    /// wants `wanted` rows with its frame (its rows, the border, and any
    /// padding). A policy that fills ignores `wanted`.
    pub fn height(self, screen: u16, wanted: u16) -> u16 {
        let wanted = match self.fit {
            Fit::Content => wanted,
            Fit::Fill => u16::MAX,
        };
        self.height.cells(screen, wanted)
    }

    /// The modal's rect, centered in `area`, for a body that wants
    /// `wanted` rows with its frame.
    pub fn rect(self, area: Rect, wanted: u16) -> Rect {
        centered(
            area,
            self.width(area.width),
            self.height(area.height, wanted),
        )
    }
}

/// A `width` x `height` rect centered in `area`, the way ratatui's
/// `Rect::centered` centers: when the space left over is odd, the extra
/// cell goes before the rect.
pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width).div_ceil(2),
        y: area.y + (area.height - height).div_ceil(2),
        width,
        height,
    }
}
