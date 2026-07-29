## Why

The TUI currently only supports keyboard navigation. Users expect to click on tabs, list items, and buttons with their mouse — especially for quick interactions where reaching for a specific key binding feels slower than just pointing and clicking.

## What Changes

- Enable crossterm mouse capture in the terminal event loop
- Handle mouse click events to select tabs, list items, and interactive elements
- Handle scroll wheel events for scrolling through lists and chat history
- Map click coordinates to UI regions (tabs bar, main list, chat area, etc.)

## Capabilities

### New Capabilities
- `mouse-interaction`: Mouse click and scroll support for the TUI — tab switching, list item selection, scrolling, and button activation via mouse events.

### Modified Capabilities

## Impact

- `crates/tui/src/main.rs`: Event loop changes to capture and dispatch mouse events
- `crates/tui/src/ui.rs`: Need to track rendered widget positions (Rect areas) so clicks can be mapped to logical elements
- `crossterm` dependency: Already supports mouse — just needs `EnableMouseCapture`/`DisableMouseCapture`
