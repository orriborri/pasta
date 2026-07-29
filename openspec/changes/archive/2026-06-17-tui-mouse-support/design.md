## Context

The pasta TUI uses ratatui 0.29 with crossterm 0.28. The event loop in `main.rs` polls for `CEvent::Key` only. The layout is a vertical split: tabs bar (height 3), main content area, and a status bar (height 1). Each tab renders lists or a chat view. crossterm already supports mouse capture — it just needs to be enabled.

## Goals / Non-Goals

**Goals:**
- Click tabs to switch between them
- Click list items to select them
- Scroll wheel to navigate lists and chat history
- Maintain all existing keyboard shortcuts unchanged

**Non-Goals:**
- Drag-and-drop
- Mouse hover highlighting
- Text selection/copy via mouse (terminal handles this when mouse capture is disabled)
- Right-click context menus

## Decisions

**1. Hit-testing via stored Rect areas**

Store the `Rect` returned by layout splits in the `App` struct (or a separate `LayoutCache`) after each `draw()` call. On mouse click, compare coordinates against these rects to determine which region was clicked.

Alternatives considered:
- Recalculating layout on click: duplicates layout logic, fragile.
- Widget-level click handlers: ratatui doesn't support this pattern.

**2. Enable mouse capture globally**

Call `EnableMouseCapture` at startup and `DisableMouseCapture` at shutdown, alongside the existing `EnterAlternateScreen`/`LeaveAlternateScreen`. This is the standard crossterm pattern.

**3. Tab hit-testing by dividing the tabs Rect evenly**

The tabs bar has 7 tabs rendered by `ratatui::Tabs`. Divide the tabs Rect width by 7 and map x-coordinate to tab index.

**4. List item hit-testing by row offset**

For list/table views, clicked row = (mouse_y - content_area.y) + scroll_offset. Already track `selected` index; clicking just sets it.

## Risks / Trade-offs

- **[Terminal compatibility]** → Some terminals handle mouse capture differently. Mitigation: crossterm handles this portably; no extra work needed.
- **[Text selection disabled]** → With mouse capture on, users can't select text with mouse. Mitigation: most terminals allow Shift+click for selection. This is standard for TUI apps (vim, htop, etc.).
- **[Layout cache staleness]** → If we store Rects from draw, they're valid until next resize. Mitigation: recalculate on every frame (draw already does this); store after each draw.
