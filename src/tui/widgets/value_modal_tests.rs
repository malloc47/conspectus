use super::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::overlay::Outcome;
use tuikit::testkit::keys::{press, typed};

fn modal(value: &str) -> ValueModal {
    open(ValueText::new("cwd", value))
}

/// `count` numbered lines.
fn numbered(count: usize) -> String {
    (1..=count)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(modal: &mut ValueModal, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    modal.draw(area, &mut buf, &Theme::default());
    buf
}

fn screen(buf: &Buffer) -> String {
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn esc_closes_the_modal() {
    let mut modal = modal("/long/path");
    assert_eq!(modal.handle_key(press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_closes_the_modal() {
    let mut modal = modal("/long/path");
    assert_eq!(modal.handle_key(typed('q')), Outcome::Dismiss);
}

#[test]
fn o_toggles_the_modal_off() {
    // Mirrors the open key so operators can press `o` twice to
    // pop the modal back closed.
    let mut modal = modal("/long/path");
    assert_eq!(modal.handle_key(typed('o')), Outcome::Close);
}

#[test]
fn j_and_k_scroll_one_row() {
    let area = Rect::new(0, 0, 80, 24);
    let mut modal = modal(&numbered(60));
    draw(&mut modal, area);
    modal.handle_key(typed('j'));
    modal.handle_key(typed('j'));
    draw(&mut modal, area);
    assert_eq!(modal.window().offset(), 2);
    modal.handle_key(typed('k'));
    draw(&mut modal, area);
    assert_eq!(modal.window().offset(), 1);
    // Scrolling above zero clamps.
    modal.handle_key(typed('k'));
    modal.handle_key(typed('k'));
    draw(&mut modal, area);
    assert_eq!(modal.window().offset(), 0);
}

#[test]
fn page_down_and_up_jump_by_the_rows_the_last_draw_showed() {
    let area = Rect::new(0, 0, 80, 24);
    let mut modal = modal(&numbered(112));
    draw(&mut modal, area);
    let shown = modal.window().viewport();
    modal.handle_key(press(KeyCode::PageDown));
    draw(&mut modal, area);
    assert_eq!(modal.window().offset(), shown);
    modal.handle_key(press(KeyCode::PageUp));
    draw(&mut modal, area);
    assert_eq!(modal.window().offset(), 0);
}

#[test]
fn g_and_capital_g_jump_to_top_and_bottom() {
    let area = Rect::new(0, 0, 80, 24);
    let mut modal = modal(&numbered(42));
    modal.handle_key(typed('G'));
    let end = screen(&draw(&mut modal, area));
    let window = modal.window();
    assert_eq!(window.offset(), window.last_offset());
    // The last page, never a blank body: the first row shows the page's
    // first line and the last line sits right above the footer's blank
    // row.
    let rows: Vec<&str> = end.lines().collect();
    let footer = rows
        .iter()
        .position(|row| row.contains("j/k/PgDn/PgUp scroll"))
        .expect("the footer");
    assert_eq!(rows[footer - 1].trim_matches(['│', ' ']), "", "{end}");
    assert!(rows[footer - 2].contains("line 42"), "{end}");
    let first = 42 - window.viewport() + 1;
    assert!(rows[2].contains(&format!("line {first} ")), "{end}");
    modal.handle_key(typed('g'));
    draw(&mut modal, area);
    assert_eq!(modal.window().offset(), 0);
}

#[test]
fn the_draw_reports_the_rows_the_value_fills() {
    let area = Rect::new(0, 0, 80, 24);
    let mut modal = open(ValueText::new("detail", numbered(40)));
    draw(&mut modal, area);
    let window = modal.window();
    // A 22-row modal: 20 inside the frame, two of them the footer and the
    // blank row above it.
    assert_eq!(window.viewport(), 18);
    assert_eq!(window.last_offset(), 40 - 18);
}

#[test]
fn y_asks_the_runtime_to_copy_the_value() {
    let mut modal = modal("/long/path");
    assert_eq!(
        modal.handle_key(typed('y')),
        Outcome::Stay(Msg::Copy {
            text: "/long/path".to_string(),
            done: "Copied text".to_string(),
        })
    );
    assert_eq!(
        modal.content().copy_payload(),
        ("/long/path", "text".to_string())
    );
}

#[test]
fn a_plan_copies_its_commands_not_its_prose() {
    let plan = ValueText::plan(
        "rebuild plan",
        "rebuild mux `a`\n  1. new session\n       $ tmux new-session -d -s a",
        "# new session\ntmux new-session -d -s a\n",
        1,
    );
    assert_eq!(
        plan.copy_payload(),
        (
            "# new session\ntmux new-session -d -s a\n",
            "1 command".to_string()
        )
    );
    let mut modal = open(plan);
    assert_eq!(
        modal.handle_key(typed('y')),
        Outcome::Stay(Msg::Copy {
            text: "# new session\ntmux new-session -d -s a\n".to_string(),
            done: "Copied 1 command".to_string(),
        })
    );
    let many = ValueText::plan("p", "t", "s", 3);
    assert_eq!(many.copy_payload().1, "3 commands");
}

#[test]
fn a_plan_draws_its_commands_in_the_accent_style_and_a_footer() {
    let theme = Theme::default();
    let plan = ValueText::plan(
        "rebuild plan",
        "rebuild mux `a`\n  1. new session\n       $ tmux new-session -d -s a",
        "tmux new-session -d -s a\n",
        1,
    );
    let area = Rect::new(0, 0, 100, 30);
    let buf = draw(&mut open(plan), area);
    let mut command_cell = None;
    for y in 0..area.height {
        let row: String = (0..area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect();
        if let Some(x) = row.find("tmux new-session") {
            let col = row[..x].chars().count() as u16;
            command_cell = Some(buf[(col, y)].style());
        }
    }
    let style = command_cell.expect("the command is drawn");
    assert_eq!(style.fg, Some(theme.accent));
    let text = screen(&buf);
    assert!(
        text.contains("j/k/PgDn/PgUp scroll · y copy · Esc/q/o close"),
        "{text}"
    );
    assert!(text.contains("┌ rebuild plan "), "{text}");
}

#[test]
fn a_long_value_wraps_beside_a_scrollbar() {
    let area = Rect::new(0, 0, 40, 12);
    let mut modal = modal(&"word ".repeat(80));
    let buf = draw(&mut modal, area);
    assert!(modal.window().overflows());
    let text = screen(&buf);
    assert!(
        text.lines().nth(2).is_some_and(|row| row.contains("word")),
        "{text}"
    );
    // The bar takes the body's last column, inside the frame.
    let bar_column = area.width - 2;
    assert!(
        (2..8).any(|y| buf[(bar_column, y)].symbol() != " "),
        "{text}"
    );
}

#[test]
fn ctrl_c_closes_modal() {
    let mut modal = modal("/long/path");
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(modal.handle_key(event), Outcome::Dismiss);
}
