# Edged 2 behavior

This document describes the app's intended interactions. Exact sizes and timing constants live with the implementation. The [original visual study](archive/design.md) is retained as historical research, not as the current specification.

## Panel

Each screen has a panel at its left or right edge. The collapsed panel leaves an icon strip visible. The panel opens when the pointer reaches it and closes after the pointer leaves, with a short grace period for crossing a corner. Full-screen windows cause the panel to tuck away.

The Unfold setting keeps icons at the screen edge while revealing titles. Slide moves the whole panel. Screen-edge corners are square; corners facing the desktop are rounded. The native window provides the backdrop and frame animation.

## Applications and Spaces

Applications without windows appear first. Windows are grouped by Space and application, then ordered by title to keep the list stable. The current Space is highlighted. Hidden applications and minimized windows are dimmed; the focused window is marked on its icon.

Clicking a window brings it forward; clicking a windowless application activates it. Space headings switch Space. Context menus offer window actions and moving a window to another desktop. Moving between Spaces follows the title-bar drag sequence because the platform does not provide a public direct API for it.

The desktop model responds to macOS notifications and periodically reconciles its list with the window server. It batches notifications and reads applications in bounded slices so discovery does not block the interface. The clearing service narrows windows that cover a visible panel strip.

## Window previews

Depending on the setting, resting on a row or holding Command while resting shows a picture at that window's frame. A frontmost visible window needs no preview. Minimized windows and windows on other Spaces can be pictured without bringing them forward.

The first row has a longer hover delay than subsequent rows. When the next picture is ready, it replaces the previous one and the preview frame follows it. Leaving the panel or activating a window dismisses the picture. Screen Recording permission is required.

## Move, Resize, and Ring

Move and Resize have separate enable switches, shortcuts, and screen-edge constraints. Holding the Move shortcut moves the window under the pointer without a mouse click. Resize follows either the bottom-right corner or the corner nearest the pointer. The opposite corner stays fixed.

Ring uses its own enable switch and shortcut. Holding the shortcut opens the ring at the pointer. Moving selects a direction and shows the proposed destination; releasing the shortcut applies it. The window does not move while a direction is being selected. Each direction can have a different destination or do nothing.

Defaults are Control–Shift for Move, Option–Shift for Resize, and Control–Command for Ring. When enabled shortcuts match, Move takes priority over Resize, then Ring. An empty shortcut disables that interaction.

## Settings and demonstrations

Settings are grouped into General, Panel, Previews, Move, Resize, Ring, and About. Disabled features retain their configured values. Ring assignment dropdowns open at the direction being edited and show destination diagrams.

The desktop demonstrations illustrate current settings without operating on real windows. Key-hold cues and pointer movement must match the real sequence, particularly Ring's release step and preview hover delays. Reduce Motion shows the result without replaying motion.

## Scope

Menu-bar item listing from the original app is intentionally omitted. Current work focuses on windows, Spaces, previews, and window placement. Platform limitations belong in [Inset gaps](inset-gaps.md).


## Updates

The Updates entity checks https://edged2.app/api/version shortly after startup when no check has been attempted that UTC day. 