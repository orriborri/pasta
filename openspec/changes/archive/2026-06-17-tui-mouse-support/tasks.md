## 1. Enable Mouse Capture

- [x] 1.1 Add `EnableMouseCapture` to terminal setup in `main.rs` (alongside `EnterAlternateScreen`)
- [x] 1.2 Add `DisableMouseCapture` to terminal teardown (alongside `LeaveAlternateScreen`)

## 2. Layout Cache

- [x] 2.1 Add a `LayoutCache` struct to `ui.rs` holding `tabs_rect: Rect` and `content_rect: Rect`
- [x] 2.2 Populate the cache in `draw()` after layout split, return it (or store in `App`)
- [x] 2.3 Pass `LayoutCache` back from `terminal.draw()` closure to the event loop

## 3. Mouse Event Dispatch

- [x] 3.1 Match `CEvent::Mouse` in the event loop (alongside existing `CEvent::Key`)
- [x] 3.2 Route mouse events to a `handle_mouse()` function that receives `MouseEvent` and `LayoutCache`

## 4. Tab Click

- [x] 4.1 In `handle_mouse()`, detect clicks in `tabs_rect` and map x-coordinate to tab index (divide width by 7)
- [x] 4.2 Set `app.tab` to the resolved tab

## 5. List Click

- [x] 5.1 Detect clicks in `content_rect` and compute row index from y-coordinate offset
- [x] 5.2 Clamp to valid item range and set `app.selected`

## 6. Scroll Wheel

- [x] 6.1 Handle `MouseEventKind::ScrollDown` / `ScrollUp` in Normal mode: increment/decrement `app.selected` clamped to list bounds
- [x] 6.2 Handle scroll in Chat mode: scroll chat message history up/down

## 7. Verify

- [x] 7.1 Build and manually test: click tabs, click list items, scroll in list, scroll in chat, verify keyboard still works
