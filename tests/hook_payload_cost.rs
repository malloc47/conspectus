//! What parsing a hook payload costs `hook write`, measured in bytes
//! allocated on the parsing thread: a `PostToolUse` whose `tool_response`
//! runs to megabytes costs what a small one does, because only the fields
//! the adapter reads are built and the rest is skipped. The payloads are
//! synthetic.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use conspectus::discovery::harness::{HarnessAdapter, registered_adapters};
use serde_json::{Value, json};

/// The system allocator, counting the bytes the current thread asks for
/// while [`allocated_by`] measures it.
struct Counting;

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

fn count(bytes: usize) {
    if MEASURING.with(Cell::get) {
        ALLOCATED.with(|total| total.set(total.get() + bytes));
    }
}

// SAFETY: every call forwards to `System` unchanged; the counter only
// reads and writes const-initialized thread-local cells, which allocate
// nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The bytes `work` allocates on this thread, and what it returned.
fn allocated_by<T>(work: impl FnOnce() -> T) -> (usize, T) {
    ALLOCATED.with(|total| total.set(0));
    MEASURING.with(|on| on.set(true));
    let value = work();
    MEASURING.with(|on| on.set(false));
    (ALLOCATED.with(Cell::get), value)
}

fn claude_code() -> &'static dyn HarnessAdapter {
    registered_adapters()
        .find(|adapter| adapter.harness_key() == "claude-code")
        .expect("the Claude Code adapter")
}

/// A `PostToolUse` of the `Read` tool whose file content is `content`,
/// with the output again as a list of content blocks, as a tool returns.
fn post_tool_use(content: &str) -> Vec<u8> {
    json!({
        "session_id": "3f2a9c1e-7b4d-4e8a-9c6f-1a2b3c4d5e6f",
        "transcript_path": "/home/user/.claude/projects/-work/3f2a9c1e.jsonl",
        "cwd": "/work",
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": "/work/big.log"},
        "tool_response": {
            "type": "text",
            "file": {"filePath": "/work/big.log", "content": content, "numLines": 1},
            "blocks": [{"type": "text", "text": content}],
        },
        "tool_use_id": "toolu_01",
    })
    .to_string()
    .into_bytes()
}

#[test]
fn a_multi_megabyte_tool_response_costs_what_a_small_one_does() {
    let small = post_tool_use("one line\n");
    // Escapes (newlines, quotes) in the text, which a parse that kept it
    // would have to unescape into a copy.
    let large = post_tool_use(&"a line of a large \"log\" file\n".repeat(1 << 17));
    assert!(large.len() > 6 << 20, "{} bytes", large.len());
    let fields = claude_code().hook_payload_fields();

    let (small_cost, small_payload) =
        allocated_by(|| conspectus::hook::payload::parse(&small, fields));
    let (large_cost, large_payload) =
        allocated_by(|| conspectus::hook::payload::parse(&large, fields));

    assert_eq!(large_payload, small_payload, "the same fields are kept");
    assert!(small_payload.is_some());
    assert!(
        large_cost <= small_cost + 4096,
        "a large tool_response allocated {large_cost} bytes, a small one {small_cost}"
    );

    // The whole-value parse it replaces builds every string in it.
    let (whole_cost, _) = allocated_by(|| serde_json::from_slice::<Value>(&large).ok());
    assert!(whole_cost > large.len(), "{whole_cost} bytes");
}
