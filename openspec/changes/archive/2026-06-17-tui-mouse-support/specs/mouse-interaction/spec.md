## ADDED Requirements

### Requirement: Mouse capture is enabled
The TUI SHALL enable crossterm mouse capture on startup and disable it on shutdown.

#### Scenario: Mouse events are received
- **WHEN** the TUI starts
- **THEN** crossterm `EnableMouseCapture` is active and `CEvent::Mouse` events are received by the event loop

#### Scenario: Mouse capture is released on exit
- **WHEN** the TUI exits (normally or via quit command)
- **THEN** crossterm `DisableMouseCapture` is called before leaving raw mode

### Requirement: Click tab to switch
The TUI SHALL switch to the clicked tab when the user clicks within the tabs bar area.

#### Scenario: Click on a tab
- **WHEN** user clicks on the area corresponding to a tab in the tabs bar
- **THEN** the active tab changes to the clicked tab

#### Scenario: Click outside tabs bar
- **WHEN** user clicks outside the tabs bar region
- **THEN** the active tab does not change

### Requirement: Click list item to select
The TUI SHALL select the clicked list item in any tab that displays a list.

#### Scenario: Click a visible list item
- **WHEN** user clicks on a row within the list content area
- **THEN** the `selected` index updates to the clicked item's index

#### Scenario: Click beyond list bounds
- **WHEN** user clicks on a row beyond the number of items in the list
- **THEN** the `selected` index does not change

### Requirement: Scroll wheel navigates lists
The TUI SHALL move the selection up or down when the user scrolls the mouse wheel over a list area.

#### Scenario: Scroll down
- **WHEN** user scrolls the mouse wheel down over the list area
- **THEN** the `selected` index increments by 1 (clamped to list length)

#### Scenario: Scroll up
- **WHEN** user scrolls the mouse wheel up over the list area
- **THEN** the `selected` index decrements by 1 (clamped to 0)

### Requirement: Scroll wheel in chat view
The TUI SHALL scroll the chat message history when the user scrolls the mouse wheel in chat mode.

#### Scenario: Scroll up in chat
- **WHEN** user scrolls up in chat mode
- **THEN** the chat view scrolls to show older messages

#### Scenario: Scroll down in chat
- **WHEN** user scrolls down in chat mode
- **THEN** the chat view scrolls toward newer messages (clamped at latest)

### Requirement: Mouse does not interfere with keyboard
All existing keyboard shortcuts SHALL continue to work unchanged when mouse capture is enabled.

#### Scenario: Keyboard still works
- **WHEN** user presses a keyboard shortcut (e.g., `1`-`7` for tabs, `j`/`k` for navigation)
- **THEN** the shortcut behaves identically to before mouse support was added
