# Edged: the original's visual design, and a design for the Inset port

Part 1 records what the Swift/Shaft app at `/Users/mac/code/Edged` actually draws, with the
line that decides each value. Paths are relative to that checkout unless marked otherwise;
Shaft's own sources are under `.build/checkouts/Shaft/`.

Part 2 is a design for the Inset port (`/Users/mac/code/edged2`) aimed at macOS 26: what to
keep, what to change, and the numbers to change it to.

---

# Part 1 — The original, as built

## 1.1 The widget tree, in one pass

```
EdgedApp                                     main.swift:34   textStyle(color #000000, leadingDistribution .even)
└ Column
  ├ Expanded { AboutView }                   main.swift:38   in the hidden root NSWindow (DesktopView.visible = false, main.swift:25)
  └ EdgedWindow(forScreen:) per screen       main.swift:42   keyed by displayID
    └ SubWindow → MouseRegion                main.swift:92   onEnter/onHover set isHovering = true, onExit false
      └ MatchContentSize(.tightFor(width:250), controlWidth:false)   main.swift:119
        ├ ActivityIndicator      while either permission is .unknown           main.swift:121
        ├ EdgedPermissionsView   while either permission is not .granted       main.swift:129
        └ EdgedView(isExpanded:) otherwise                                     main.swift:124
          └ Column(mainAxisSize:.min, crossAxisAlignment:.start)  main.swift:179  .textStyle(fontSize: 13) main.swift:286
            ├ "M" SectionIndicator + horizontal ListView of menu-bar icons     main.swift:185-211
            ├ Row(spaceBetween) { "A" SectionIndicator, chevron-down }          main.swift:213-231
            ├ ClickableRow per application with no windows                      main.swift:233-248
            └ per Space: buildSpaceSection(...) wrapped in the current-Space decoration  main.swift:250-284
```

`MatchContentSize` lays the content out at a **tight 250 pt width at all times** and reports only
the height to the native view (`controlWidth: false`, main.swift:119; `MatchContentSize.swift:93-104`,
`108-128`). The window's width is owned by `WindowSlideManager`. So the collapsed strip is not a
different layout — it is the **leftmost 28 pt of the same 250 pt layout**, with the rest outside
the window.

## 1.2 Window chrome — `Sources/MacOS/Helpers/NSWindow.swift`

| Property | Value | Line |
|---|---|---|
| Class | `PalettePanel: NSPanel`, `canBecomeKey = true` | 62-64 |
| `isFloatingPanel` | `true` | 5 |
| `hidesOnDeactivate` | `false` | 7 |
| `becomesKeyOnlyIfNeeded` | `true` | 8 |
| `level` | `.popUpMenu` (101) | 9 |
| `styleMask` | `[.nonactivatingPanel]` | 11 |
| `preventsActivation` | `true`, set by KVC | 16 |
| `collectionBehavior` | `[.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]` | 19-23 |
| Corner radius | `8` on `contentView.layer` | 12 |
| Corners masked | `[.layerMinXMinYCorner, .layerMinXMaxYCorner]` — **left two only** | 15 |
| `masksToBounds` | `true` (this is what clips overflowing titles) | 13 |
| `titlebarAppearsTransparent` | `true`; the three standard buttons hidden | 29-36 |
| Appearance | **hard-coded** `NSAppearance(named: .aqua)` — light, always | 39 |
| `backgroundColor` / `isOpaque` | `.clear` / `false` | 42-43 |
| Material | `NSVisualEffectView`, `blendingMode = .behindWindow`, `state = .active`, `material = .popover` | 45-49 |
| Blur view corners | radius `8`, same two left corners | 51-52 |
| Shadow | never set → AppKit's default window shadow (`hasShadow` defaults true) | — |
| Initial content size | `200 × 400`, immediately overridden by `MatchContentSize` and the slide manager | 55 |

Opacity of the window itself is `1.0` except in a full-screen Space, where it is set to `0.0`
while hidden (`WindowSlideManager.swift:303-309`).

## 1.3 Slide geometry and timing — `Sources/MacOS/WindowSlideManager.swift`

Constructed with `visibleEdgeWidth: 28.0`, `fullWidth: 250`, `showDuration: 0.1`,
`hideDuration: 0.1` (main.swift:76-83).

- Hidden frame: `x = visibleFrame.maxX - 28`, `width = 28`, `y = visibleFrame.midY - height/2`
  (352-368). In a full-screen Space the width becomes `1` instead of 28 (359).
- Visible frame: `x = visibleFrame.maxX - 250`, `width = 250`, same `y` (372-385).
- Height is whatever the content measured; there is no cap, so a tall list runs off both ends of
  the screen.
- Animation: `NSAnimationContext.runAnimationGroup`, `duration = 0.1`,
  `timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)`, animating
  `window.animator().setFrame(...)` (244-259). Cancelling sets `duration = 0` (262-268).
- **The right edge is fixed and the left edge moves.** Because the content is pinned to the
  window's left edge (1.1), every row travels 222 pt to the left over those 100 ms as the panel
  opens, and back again when it closes. The icons do not stay under the pointer; they fly.
- Hover is edge-triggered by `MouseRegion` (main.swift:93-117) and re-checked by polling:
  `WindowVisibilityMonitor(pollInterval: 0.5)` (88-96), full-screen detection every `0.5 s`
  (390-399), `WindowOverlapManager(pollInterval: 3.0)` narrowing windows that cover the strip
  (`WindowOverlapManager.swift:26`).

## 1.4 The collapsed strip, and why the icon is centred in it

There is no strip-specific code. The centring is arithmetic that happens to work out:

1. `IconView(icon:)` with no target size renders the icon into a **48 × 48 pixel** texture —
   `defaultSize: Float = 48` (`View/IconView.swift:77`, used at 89-91).
2. It is displayed as `RawImage(image:, scale: View.maybeOf(context)?.devicePixelRatio ?? 1)`
   (`IconView.swift:42-46`). `RenderImage` sizes itself as `image.width / scale`
   (`Shaft/Sources/Shaft/Rendering/RenderImage.swift:303-304`), and `devicePixelRatio` is the
   display scale (`Shaft/Sources/ShaftSDL3/SDLView.swift:128-139`).
3. On a 2× display that is **24 × 24 pt**.
4. `DefaultClickableRowStyle.build` wraps the row in `.padding(.all(2))`
   (`View/ClickableRow.swift:159`).

So `2 + 24 + 2 = 28`, exactly `visibleEdgeWidth` — the icon sits with 2 pt of gutter on each side,
and the row is 28 pt tall for the same reason. Neither 28 nor 2 nor 48 knows about the others.

**This breaks on a 1× display**: `devicePixelRatio == 1` gives a 48 pt icon, a 52 pt row, and an
icon 20 pt wider than the strip. It also means the icon's logical size is not a design decision
anyone can find in the source.

The menu-bar section takes `targetHeight: 48` explicitly (main.swift:191) inside a
`ListView(scrollDirection: .horizontal).constrained(width: .infinity, height: 24)`
(main.swift:189, 209) — same 24 pt result on a 2× display, widths by aspect ratio, no spacing
between items.

## 1.5 Row layout — `Sources/View/ClickableRow.swift` and main.swift:353-385

```
Row(crossAxisAlignment: .center) { icon-stack, SizedBox(width: 2), Text(title) }   main.swift:341-345
  .textStyle(.init(color: textColor))    ClickableRow.swift:158
  .padding(.all(2))                      ClickableRow.swift:159
  .decoration(.box(color: color))        ClickableRow.swift:160
```

- Padding: **2 pt on all four sides**, hard-coded. `DefaultClickableRowStyle.init(padding:)` exists
  (127) and is stored (131) but `build` never reads it, so the
  `DefaultClickableRowStyle(padding: .only(top: 2, right: 2, bottom: 2))` passed for Space sections
  (main.swift:350) is **silently ignored**.
- Icon: 24 pt (1.4). Gap to the title: `SizedBox(width: 2)` (main.swift:244, 343). Title starts at
  x = 28 — flush with the strip's right edge.
- Row height: 28 pt (2 + 24 + 2); the 13 pt text is shorter than the icon and does not drive it.
- Font: size **13** from `.textStyle(.init(fontSize: 13))` on the whole list (main.swift:286),
  weight unset (regular), **no font family is named anywhere in the app**, so the renderer's
  fallback face is used rather than San Francisco explicitly.
- `Row` defaults to `mainAxisSize: .max` (`Shaft/.../Basic.swift:794-814`), so the row and
  therefore the fill **span the full 250 pt**, including the part under the collapsed strip.
- Fill colours (`ClickableRow.swift:133-161`), all hard-coded ARGB:

| State | Fill | Text |
|---|---|---|
| selected **and** hovered and enabled | `0xDD0A84FF` (system blue, 87 %) | `#FFFFFF` |
| selected | `0x55E5E5E5` (33 %) | `#000000` |
| hovered and enabled | `0xCC0A84FF` (system blue, 80 %) | `#FFFFFF` |
| otherwise | `0x00000000` | `#000000` |

- **Corner radius of the fill: none.** `.box(color:)` with no `borderRadius` is a square,
  full-width block (160).
- **The two "selected" branches are dead code.** Every `ClickableRow` in the app is built as
  `ClickableRow(isDisabled: !isExpanded)` (main.swift:237, 320) and never passes `isSelected`, which
  defaults to `false` (ClickableRow.swift:7). The focused window is marked only by the icon ring
  (1.8).
- Disabled: `isEnabled = onPressed != nil && !isDisabled` (45) suppresses the hover fill only.
  `onTapUp` calls `widget.onPressed?()` **regardless of `isDisabled`** (57-60), so a click on the
  collapsed strip still activates the row.
- Pressed: tracked in `isPressed` (51-56) and never used by the style — no pressed feedback.
- Focus: the row is wrapped in `Focus` (72) but `isFocused` is hard-coded `false` (43) — no focus
  ring is possible.
- Cursor: `.system(.click)` (71).

## 1.6 Section markers — `SectionIndicator`, main.swift:473-493

```
Text(text).center().constrained(width: 15, height: 15)                       484
  .textStyle(color #4F4F4F, fontSize 11, fontWeight .bold)                   485
  .decoration(.box(border: Border.all(#4F4F4F, width: 1), borderRadius: .circular(4)))  488-489
```

- Box 15 × 15, 1 pt border, radius 4, label 11 pt bold, both border and label `#FF4F4F4F`.
- Letters: `"M"` for menu-bar items (186), `"A"` for applications without windows (214),
  the Space number as `index + 1`, or `"F"` when `space.index` is nil, which is how a full-screen
  Space is recorded (main.swift:295-300, `MacOS/Spaces.swift:74-86`).
- Placement, all three: `.padding(.only(left: 6, top: 4, bottom: 4))` (187, 215, 306) — so the box
  occupies x ∈ [6, 21], **centre 13.5**, while icons occupy x ∈ [2, 26], **centre 14**. The two
  columns are half a point apart and different widths; they do not share a grid.
- Vertical: 4 above + 15 + 4 below = a 23 pt block; rows are 28 pt; nothing else separates sections.
- A Space marker is stretched full width before being padded, and the whole strip is the click
  target for switching Space: `.align(.centerLeft).constrained(width: .infinity).boxDecoration()
  .padding(...).gesture(onTap: space.switchTo)` (302-312). `.boxDecoration()` with no arguments
  paints nothing.

## 1.7 The current-Space container — main.swift:262-283

Applied to the whole `buildSpaceSection` column when
`Spaces.shared.currentSpace(for: screen) == space` (251-253):

- Fill `0x55E5E5E5` — white-grey at 33 % (265).
- `borderRadius: .circular(8)` (271).
- One shadow: colour `0x55000000`, offset `(0, 0)`, blur radius `2`, spread `-1`,
  `blurStyle: .outer` (272-280).
- **No padding**: the fill starts at the top of the marker's 4 pt padding and ends at the bottom of
  the last row. Width is the full 250 pt, because the marker inside is `.constrained(width: .infinity)`.
- No animation: the highlight jumps between sections on rebuild.

## 1.8 The focused-window ring — main.swift:353-366

```
Stack(clipBehavior: .none) {                                                  354
  IconView(icon:)                                                             355
    .decoration(isSelected ? .box(border: Border.all(color: 0xEEE5E5E5, width: 3,
                                    strokeAlign: .strokeAlignInside),
                                  borderRadius: .circular(7)) : .box())       356-365
```

- Condition: `window.application.isActive && (focusedWindow === window || windows.count == 1)`
  (main.swift:315-318).
- Colour `#E5E5E5` at 93 % — a light grey that is invisible on a light material, and 3 pt wide,
  which is heavy on a 24 pt icon.
- **It is painted behind the icon.** `Widget.decoration(_:position:)` defaults to
  `DecorationPosition.background` (`Shaft/.../Container.swift:55-62`), so the ring is drawn first
  and the icon on top of it; only the part under the icon's transparent margin shows.

## 1.9 Dock badges — main.swift:368-381

```
Positioned(top: -3, right: -2) {                                              369
  Text(dockLabel)
    .textStyle(color #FFFFFF, fontSize 10)                                    372
    .padding(.symmetric(vertical: 0, horizontal: 4))                          374
    .decoration(.box(color: .argb(255, 221, 76, 61), borderRadius: .circular(16)))  377-378
}
```

- Fill `#DD4C3D`, hard-coded — not `NSColor.systemRed` and not the Dock's red.
- Shape: radius 16 on a box whose height is the 10 pt line box (~12 pt), so it is a capsule by
  accident; width is the text plus 8.
- Position: 3 pt above and 2 pt right of the 24 pt icon box, in a `Stack(clipBehavior: .none)`, and
  `Row`/`Column` also default to `Clip.none` — so the badge overhangs into the row above and, at the
  right, past x = 28, where the collapsed strip clips it.
- The text is whatever `NSRunningApplication`'s dock tile reports (`MacOS/Applications.swift:169`);
  there is no numeric parsing, no cap, no dot form for non-numeric labels.

## 1.10 The chevron and the header row — main.swift:213-231

`Row(mainAxisAlignment: .spaceBetween)` at full 250 pt width, holding the "A" marker and
`LucideIcon("chevron-down").padding(.only(right: 6))`. `LucideIcon` defaults to **size 14** and
takes its colour from the ambient text style (`Shaft/.../ShaftLucide/LucideIcon.swift:81, 105`) —
here `#000000`. So the glyph is 14 × 14 at x ∈ [230, 244]: **outside the collapsed strip**, and
invisible until the panel is open. It has no hover state, no fill, no pressed state; the hit area is
the glyph plus its 6 pt right padding. `onTapDown` opens the native menu at the global pointer
position (228) — Launch at Login, Quit Edged (454-470).

## 1.11 Text truncation, dimming, scrolling

- **Truncation: none.** The title `Text` is not wrapped in `Expanded`/`Flexible`, and `RenderFlex`
  gives a non-flex child in a horizontal flex `BoxConstraints(maxHeight:)` — unbounded width
  (`Shaft/.../Rendering/RenderFlex.swift:421-438`). `maxLines` and `overflow` are never set, so the
  default is `softWrap: true, overflow: .clip` (`Shaft/.../Basic.swift:1470-1471`). A long title is
  laid out at its full width, paints past 250 pt, and is cut by the window's
  `masksToBounds` — a hard edge mid-glyph, with no ellipsis and no fade.
- **Dimming: none.** `Window.isMinimized` and `Application.isHidden` exist
  (`MacOS/Window.swift:13`, `MacOS/Application.swift:29`) and are used only by
  `WindowOverlapManager`; nothing in the view reads them. Minimized windows and hidden applications
  look exactly like live ones.
- **Scrolling: none** in the panel. The only scroll view is the horizontal menu-bar `ListView`
  (main.swift:189). The window's height is the content's height and it is centred on
  `visibleFrame.midY`, so past roughly 30 rows the list simply extends off both ends of the screen.

## 1.12 The permissions pane — `Sources/View/EdgedPermissionsView.swift`

Shown in place of the list, at the same 250 pt width, whenever Accessibility or Screen Recording is
not granted (main.swift:121-130); an `ActivityIndicator` while either is `.unknown`.

- Container `.padding(.all(16))` (61), a plain `Column` (defaults: `mainAxisSize .max`, so it fills
  the panel height).
- Title "Permissions Required": 14 pt bold `#000000` (9-15).
- Gaps: 16 below the title, 8 between the two rows, 16 above "Quit" (17, 30, 43).
- `PermissionRow` (65-97): `LucideIcon(name)` at the default 14 pt with `.padding(.only(right: 8))`,
  an `Expanded` label at 13 pt `#333333`, then either `LucideIcon("circle-check", color: #34C759)`
  or a `Button("Grant")` with `.textStyle(fontSize: 12)`.
- `Button` uses `DefaultButtonStyle` (`Shaft/.../ShaftKit/Button.swift:178-220`): fill
  `rgb(0,122,255)`, white label, regular size padding `3 × 7`, radius `5`, two soft shadows.
- "Quit": 12 pt `#666666`, padding `2 × 10`, no fill, no hover — a bare word that happens to be
  tappable (45-58).
- Nothing here is legible in the 28 pt strip; the pane only exists once the pointer opens the panel.

## 1.13 Every animation in the app

| What | Duration | Curve | Where |
|---|---|---|---|
| Panel slide out (28 → 250 pt) | 0.1 s | `CAMediaTimingFunction(.easeInEaseOut)` | WindowSlideManager.swift:76-83, 251-255 |
| Panel slide in (250 → 28 pt) | 0.1 s | same | same |
| Re-position after a content resize | 0.1 s | same | WindowSlideManager.swift:311-329 (called from main.swift:132-134) |
| Full-screen hide | instant (`alphaValue = 0`) | — | WindowSlideManager.swift:303-309 |
| Hover fill | **none** — the colour swaps on the frame after `setState` | — | ClickableRow.swift:62-68, 133-161 |
| Current-Space highlight | **none** — jumps | — | main.swift:262-283 |
| Rows appearing/disappearing | **none** — the list and the window height jump | — | main.swift:233-284 |
| Focus ring | **none** | — | main.swift:356-365 |

There are no fades anywhere, and `Shaft`'s implicit-animation widgets are not used.

## 1.14 What comes from the system, and what does not

**From the system:** application icons and names (`NSRunningApplication.icon`,
`MacOS/Application.swift:50-51`); Dock badge text (`MacOS/Applications.swift:169`); screens,
Spaces, windows, which Space a screen shows; the `NSVisualEffectView` material; the default window
shadow; the renderer's fallback font face.

**Hard-coded, and therefore wrong in half the world's settings:**

- The **appearance is forced to Aqua** (`NSWindow.swift:39`). Dark Mode changes nothing.
- The **accent colour is never read.** `NSColor.controlAccentColor` appears nowhere; hover uses
  `#0A84FF`, which is macOS's *dark-mode* system blue, on a window forced to light.
- Every other colour is a literal: `#000000`, `#4F4F4F`, `#E5E5E5`, `#333333`, `#666666`,
  `#34C759`, `#DD4C3D`, `rgb(0,122,255)`.
- Font sizes 13/14/12/11/10 are literals; no font family, so no explicit San Francisco, and no
  response to the user's text-size settings.

## 1.15 Defects worth naming before redrawing anything

1. Forced light appearance; black text on a light material over a possibly dark desktop (1.2).
2. Accent blue used for **hover** (1.5). On macOS, accent means *selected*; sweeping the pointer
   down the list makes every row look chosen in turn.
3. Two selection languages, one of them dead: `isSelected` is never passed (1.5), so the focused
   window is signalled only by a ring that is painted behind the icon and is nearly invisible (1.8).
4. The hover fill is a square, full-bleed 250 pt block (1.5) — nothing on macOS 26 looks like that.
5. `DefaultClickableRowStyle.padding` is accepted, stored, and ignored (1.5).
6. The icon's logical size is `48 / devicePixelRatio` (1.4): invisible arithmetic, and broken at 1×.
7. The section-marker column (centre 13.5) and the icon column (centre 14) are not the same column
   (1.6).
8. Long titles are cut mid-glyph with no ellipsis (1.11).
9. Minimized windows and hidden applications are not dimmed (1.11).
10. No scrolling and no height cap: a long list leaves the screen (1.11).
11. Content is pinned to the window's left edge, so opening the panel throws every row 222 pt
    sideways in 100 ms (1.3).
12. A click on a row in the collapsed strip fires `onPressed` even though the row draws as disabled
    (1.5).
13. Hover state is corrected by 0.5 s polling (1.3), so leaving the panel can take half a second to
    register.
14. The badge overhangs into the row above and is clipped by the strip (1.9).
15. The chevron lives at x ∈ [230, 244] — unreachable until the panel is already open (1.10).
16. `hideWindowImmidiate` (WindowSlideManager.swift:332) is misspelled in the public API.

---

# Part 2 — A design for the Inset port, for macOS 26

Everything below is stated in points, at the panel's 250 pt full width and 28 pt strip width, both
kept from the original. Where the port (`crates/edged/src/`) already does the right thing it is
marked *(already in the port)*.

## 2.1 One grid, and the arithmetic that holds it

**Rule: the strip is a column, not a crop.** Define one constant, `STRIP = 28`, and give every row,
heading and control a leading cell exactly `STRIP` wide with its content centred in it. Then the
collapsed panel is that cell, exactly, and nothing needs to know it is collapsed.

```
LEADING CELL = 28          the collapsed strip
  icon box   = 24          2 pt of gutter each side: (28 − 24) / 2 = 2
  marker box = 18          5 pt of gutter each side: (28 − 18) / 2 = 5
  centre     = 14          icons and markers share it, exactly
ROW HEIGHT   = 28          = 2 + 24 + 2, the icon box plus its gutters
```

The icon is a **24 × 24 box given explicitly** (`SizedBox::new().width(24.0).height(24.0)`), never a
texture size divided by a scale factor. *Departure from the original: the 24 pt comes from the
design, not from `48 / devicePixelRatio`, so a 1× display draws the same panel (1.4).*
*Departure from the port: `ICON_SIDE` becomes 24 with the ring painted over it rather than 20 with
2 pt of ring padding (`rows.rs:25`), which recovers the original's icon size.*

Spacing scale: **2, 4, 6, 8, 12, 16**. Nothing else.

| Element | Value | vs. original |
|---|---|---|
| Panel width | 250 | same |
| Strip width | 28 | same |
| Row height | 28 | same, now stated once |
| Icon box | 24 | same size, explicit |
| Icon ↔ title gap | 6 | was 2 (main.swift:244) — 2 pt crowds the icon |
| Title column | 210 (250 − 28 − 6 − 6 trailing) | was unbounded |
| Marker box | 18, radius 5 | was 15, radius 4 |
| Heading block height | 22 (2 + 18 + 2) | was 23 (4 + 15 + 4) |
| Space above a heading | 6 | was 4 |
| Space below a heading | 2 | was 4 |
| Row pill inset, leading | 4 | was 0 |
| Row pill inset, trailing | 2 | was 0 |
| Row pill radius | 8 | was 0 |
| Space group radius | 10 on the leading corners, 0 on the trailing | was 8 all round |
| Panel radius | 12 on the leading corners, 0 on the trailing | was 8 |

The radii are concentric outwards from the icon: icon art ≈ 5.5 → ring 6 → marker 5 → row pill 8 →
group 10 → panel 12.

## 2.2 Which way the panel unfurls

The original pins the content to the window's **left** edge, so opening it drags every icon 222 pt
across the screen (1.3). Two ways out; the first is the design, the second is the fallback.

**A — Anchor the content to the trailing edge (recommended).** Lay the content out at 250 pt and
align it right inside the window (`OverflowBox`/`Align` with `AlignmentGeometry::CENTER_RIGHT`), and
put the leading cell at the **trailing** end of each row: `[ title ][ 6 ][ icon 24 ][ 2 ]`. The icon
column then sits at x ∈ [224, 248] of the layout — under the pointer at the screen edge — and stays
there while the panel widens. Nothing moves; the titles are uncovered.

Row geometry, measured in the 250 pt layout:

```
x:   0    4                     218  224        248 250
     |    |                       |    |          |  |
     |    | title, right-aligned  | 6  | icon 24  |2 |
     |<-------- row pill 4 .. 248, radius 8 ------->|
                                  |<--- 28 pt leading cell --->|
```

The title is **right-aligned, one line, tail ellipsis**, so the string hugs the icon and you read
the start of the window's name; the ellipsis lands on the leading side, away from the edge.

Cost, stated plainly: `[title][icon]` is mirrored from the usual `[icon][title]`. It is the right
mirror for a right-edge panel — macOS's own right-side Dock puts icons at the edge and labels
inboard — but it is the biggest single change here, and everything else in this document works
without it.

**B — Keep `[icon 24][6][title]` left-anchored.** Then the icon column is x ∈ [0, 28], the title
column x ∈ [34, 242], and the content still flies 222 pt on every open. If you take B, drop the
travel by keeping the window at 250 pt permanently and animating an inner container's width
instead — which needs the host to make the transparent region click-through, and belongs in
`docs/inset-gaps.md` beside the `NSPanel` item.

**The hover pill follows the strip.** Its leading inset animates with the slide: **224 → 4** on open
and back on close, on the same duration and curve (2.6). Collapsed, the pill is exactly the icon
box — a 24 pt rounded square under the icon; open, it is the full row. One property, two readings.
*New: the original suppresses the fill entirely while collapsed (`isDisabled: !isExpanded`,
main.swift:237) while still firing the click.* Make the row live in both states and drop
`isDisabled`.

## 2.3 Material and appearance

- **Background: Liquid Glass.** `WindowBackground::Glass` *(already in the port,
  `panel.rs:55`)*, i.e. `NSGlassEffectView` on macOS 26, falling back to `NSVisualEffectView` with
  material **`.sidebar`**, `blendingMode = .behindWindow`, `state = .active`. *Departure: the
  original uses `.popover` (NSWindow.swift:48), the material for something that appears and goes.
  This panel is furniture; `.sidebar` is the material for furniture.*
- **Corners:** radius **12** on the two leading corners, 0 on the trailing pair — the window is
  flush with the screen edge. *(The port has 8 all round, `panel.rs:35`.)*
- **Shadow:** the system window shadow, on. It separates the strip from whatever is behind it and it
  is the only thing that makes a 28 pt sliver read as raised. *(The port sets `shadow: false`,
  `panel.rs:56`; turn it on once the surface carries alpha.)*
- **Appearance: follow the system.** Read `NSApp.effectiveAppearance`, rebuild on
  `AppleInterfaceThemeChangedNotification`, and choose the palette from `Brightness` *(already in
  the port, `theme.rs:29`)*. The original's forced Aqua (NSWindow.swift:39) is the single worst
  thing about how it looks in 2026.
- **Accent: read it.** `NSColor.controlAccentColor` *(already in the port,
  `theme.rs:67-77`)*, and repaint on `AppleColorPreferencesChangedNotification`.
- **Fill behind the glass:** none. Let the glass be the background; a `panel` colour at `0xF2` alpha
  (`theme.rs:38`, `theme.rs:49`) defeats the material once transparency lands. Keep an opaque fallback only for as
  long as the surface is opaque, and gate it on that.

## 2.4 Hover, selection and focus against glass

Over a translucent background, a tint reads as a *shade*, not a colour: keep the fills neutral and
low, and spend the accent on the one thing that is actually selected.

| State | Light | Dark | Label |
|---|---|---|---|
| Rest | transparent | transparent | `labelColor` — `rgba(0,0,0,0.85)` / `rgba(255,255,255,0.85)` |
| Hover | `rgba(0,0,0,0.055)` | `rgba(255,255,255,0.085)` | unchanged |
| Pressed | `rgba(0,0,0,0.10)` | `rgba(255,255,255,0.14)` | unchanged |
| Focused window | accent @ **16 %** | accent @ **24 %** | unchanged |
| Focused + hover | accent @ **26 %** | accent @ **34 %** | unchanged |
| Dimmed (minimized / hidden) | icon at 45 % opacity | same | `secondaryLabel` — `rgba(0,0,0,0.50)` / `rgba(255,255,255,0.55)` |
| Keyboard focus | 2 pt `keyboardFocusIndicatorColor` ring outside the pill, radius 10 | same | unchanged |

*Departures.* The original paints **hover** in 80 %-opaque system blue with white text
(ClickableRow.swift:145-148), which reads as "selected" to anyone who uses macOS, and does it to
every row the pointer crosses. Here hover is a neutral shade and the accent marks the frontmost
window, which is the only durable selection the panel has. *Departure from the port:
`rows.rs:161-164` paints hover with the full accent and `on_accent` text — same objection.*

Semantic names to prefer, with the RGBA above as the fallback where Inset cannot reach them:
`labelColor`, `secondaryLabel`, `tertiaryLabel`, `controlAccentColor`,
`keyboardFocusIndicatorColor`, `systemRed`, `systemGreen`, `separatorColor`.

**The focus ring stays**, because it is the only focus signal visible in the collapsed strip:
1.5 pt, **accent**, `strokeAlignInside`, radius 6, drawn **over** the 24 pt icon box, not behind it.
It overlaps the icon's own transparent margin and so costs no layout. *Departure: the original's
3 pt `#E5E5E5` ring is painted at `DecorationPosition.background` (1.8) — too thick, near-invisible,
and behind the art.*

## 2.5 Section markers, and showing the current Space

- **Marker: 18 × 18, radius 5, 11 pt W600**, centred in the 28 pt leading cell so it shares the
  icon column's centre exactly (2.1). *Departure: the original's 15 pt box at `left: 6` misses the
  icon centre by half a point and is a different width (1.6).*
- **Resting marker:** 1 pt border and label in `secondaryLabel`, no fill.
- **Current Space:** the marker is **filled with the accent**, label `white`, no border. That is the
  strongest, smallest mark available, and it is legible in the 28 pt strip, which the original's
  group fill is not — *this is the change that makes the collapsed strip readable: a column of icons
  with a numbered accent chip at the head of the group you are in.*
- **Group container:** the current Space's section keeps a container, but as a quiet band, not a
  card: fill `rgba(255,255,255,0.55)` light / `rgba(255,255,255,0.10)` dark, **no shadow**, radius
  10 on the leading corners and 0 on the trailing, full bleed to the trailing edge, 4 pt of padding
  above and below. *Departure: drop the original's `0x55000000` outer shadow (main.swift:272-280) —
  a drop shadow on a translucent surface is a 2013 idiom and it muddies glass.*
- **Headings are targets:** the whole heading block switches Space; give it the same neutral hover
  fill as a row, radius 6, on a 22 × 22 box around the marker. Tooltip: "Switch to Desktop *n*"
  *(already in the port, `sections.rs:108-112`)*.
- Labels: the Space number, `F` for a full-screen Space, `A` for the applications without windows.
  **Drop `M`** — macOS 26 manages hidden menu-bar items itself *(already dropped in the port)*.
- **The chevron moves into the leading cell** of the header row, so it is reachable in the collapsed
  strip: a 24 × 22 hit target centred on the icon column, glyph 11 pt `secondaryLabel`, neutral
  hover fill, radius 6. *Departure: the original hides it at x ∈ [230, 244], where the pointer can
  only reach it after the panel is open (1.10).* Put the "A" marker inboard of it, or drop the "A"
  marker entirely and let the chevron head the list — the applications section needs no letter when
  it is always first.

## 2.6 Motion

| What | Duration | Curve | Note |
|---|---|---|---|
| Slide out (28 → 250) | **160 ms** | `Curves::ease_out_cubic` | opens on pointer arrival, no delay |
| Slide in (250 → 28) | **140 ms** | `Curves::ease_in_out_cubic` | after a **220 ms** grace, so crossing a corner does not close it |
| Row pill leading inset (224 → 4) | same as the slide | same as the slide | one animation, two properties (2.2) |
| Hover fill on | **80 ms** | `Curves::ease_out_cubic` | |
| Hover fill off | **140 ms** | `Curves::ease_out_cubic` | slower out reads calmer under a moving pointer |
| Pressed fill | **50 ms** | `Curves::ease_out_quad` | |
| Focus ring colour | **180 ms** | `Curves::ease_out_cubic` | when the frontmost window changes |
| Current-Space highlight moving | **220 ms** | `Curves::ease_in_out_cubic` | cross-fade of the two group fills and the two markers |
| Row appearing | **180 ms** | `Curves::ease_out_cubic` | height 0 → 28 with opacity 0 → 1 |
| Row disappearing | **140 ms** | `Curves::ease_in_out_cubic` | height 28 → 0 with opacity 1 → 0 |
| Window height following the content | same as the row | same as the row | so the list and its window grow together |
| Anything while collapsed | **0 ms** | — | nobody is looking; a jumping strip is worse than a jump |
| Reduce Motion is on | **0 ms** for movement, 120 ms cross-fades | `Curves::linear` | read `NSWorkspace.accessibilityDisplayShouldReduceMotion` |

*Departures.* The original runs both slides at 100 ms `easeInEaseOut` (1.13) and animates nothing
else at all. 160 ms out is slower than 100 but reads faster, because `ease_out_cubic` puts the
motion at the front; `easeInEaseOut` over 100 ms is mostly acceleration. The 220 ms close grace is
new and is what makes an edge panel tolerable to live with. *The port's 100 ms `SLIDE`
(`panel.rs:34`), 90 ms `HOVER` (`rows.rs:33`) and 160 ms `HIGHLIGHT` (`sections.rs:23`) become the
numbers above.*

## 2.7 Titles, dimming, badges, scrolling

**Titles.** One line, `TextOverflow::Ellipsis`, `max_lines(1)`, in a flex cell so the ellipsis has a
width to work against *(already in the port, `rows.rs:280-286`)*; a tooltip carries the full title
*(already in the port, `rows.rs:139`)*. 13 pt W400. *Departure: the original has no truncation at
all and cuts mid-glyph at the window edge (1.11).*

**Dimming.** A minimized window or a hidden application: **icon at 45 % opacity, label in
`secondaryLabel`** — not a blanket `Opacity` over the whole row, which turns text over glass to
mush. *Departure from the original, which does not dim at all (1.11); refinement of the port's
whole-row `Opacity(0.45)` (`rows.rs:31, 181-183`).*

**Badges.** `systemRed` (light `#FF3B30`, dark `#FF453A`), white **10 pt W700**, height **14**,
minimum width 14, radius 7 (a true capsule at any width), horizontal padding 4. Numeric labels only;
anything else becomes an **8 pt dot**; counts over 99 become `99+`. Anchored to the icon box's
**trailing** edge and grown leftwards, `top: -2, right: -1`, so it never leaves the 28 pt cell and is
never clipped by the strip. *Departures: the original's `#DD4C3D` is not the system red; `right: -2`
pushes it past the strip's edge; there is no dot form and no cap (1.9). The port's `top: -4,
right: -5` (`rows.rs:241-243`) has the same clipping problem.*

**Scrolling.** Cap the window at `visibleFrame.height − 16` *(already in the port, `panel.rs:39,
64-70`)* and put the content in a scroll view with overlay scrollers: 8 pt of padding top and
bottom, scroller hidden while collapsed, and the offset kept so the current Space's group stays in
view when the panel opens. The strip scrolls with it — it is the same column. *Departure: the
original never scrolls and lets a long list leave the screen (1.11).*

## 2.8 The permissions pane

It is the first thing a new user sees, and in the original it is invisible until they happen to
hover the edge. Make it:

- **Pinned open** while Accessibility is missing — the panel stays at 250 pt and does not slide in.
  A 28 pt strip cannot say "this app needs permission". Collapse it the moment the grant lands, with
  the normal slide.
- Title **15 pt W600** `labelColor`; one line of 13 pt `secondaryLabel` under it saying why
  ("Edged reads the windows of other applications"); 16 pt padding; 12 pt between rows.
- Each row: a 16 pt symbol in the leading cell, a 13 pt label, and either a `systemGreen` checkmark
  or a **Grant** button (13 pt, accent fill, radius 6, padding 4 × 10).
- "Quit" as a real borderless button with a hover fill, 11 pt `secondaryLabel`, not a bare word
  (1.12).
- While the status is unknown, a spinner in the same 250 pt frame — but leave the panel pinned, so
  the frame does not jump when the answer arrives.

## 2.9 What to drop from the original outright

1. The forced light appearance (1.2).
2. Accent-coloured hover (1.5) — accent is for the focused window only.
3. The dead `isSelected` path (1.5): one selection language, expressed once.
4. The square full-bleed row fill (1.5): a 244 pt pill with radius 8, insets 4 and 2.
5. The 3 pt grey ring painted behind the icon (1.8): 1.5 pt, accent, in front.
6. `48 / devicePixelRatio` as an icon size (1.4): state the 24.
7. The `padding` parameter that is stored and never read (1.5).
8. The `M` menu-bar section (1.4): macOS 26 does this itself.
9. The drop shadow under the current-Space group (1.7).
10. `isDisabled` on rows (1.5): the strip is live, and so is its hover fill.
11. Hover correction by 0.5 s polling (1.3): track the pointer with events; poll only what AppKit
    will not tell you.
12. The chevron parked outside the strip (1.10).
13. `hideWindowImmidiate` (1.15) — whatever it is called in Rust, spell it.

## 2.10 The whole spec as numbers

```
panel            250 wide, radius 12 leading / 0 trailing, glass, system shadow, system appearance
strip            28 wide, = the leading cell of every row
row              28 high; pill x 4..248 open, 224..248 collapsed, radius 8
icon             24 box at x 224..248 (trailing-anchored) — centre of the 28 pt cell
ring             1.5 accent, inside stroke, radius 6, painted over the icon
badge            h 14, radius 7, pad 4, 10 W700 white on systemRed, top -2 right -1, grows leftwards
title            13 W400, right-aligned, 1 line, tail ellipsis, column 210 wide, 6 from the icon
marker           18 box, radius 5, 11 W600; resting: border+label secondaryLabel; current: accent fill, white label
heading          22 high, 6 above, 2 below, hover fill radius 6
group            current Space only: white 55 % / 10 %, radius 10 leading / 0 trailing, 4 pad, no shadow
hover            black 5.5 % / white 8.5 %      pressed  black 10 % / white 14 %
focused          accent 16 % / 24 %             + hover  accent 26 % / 34 %
dimmed           icon 45 % opacity, label secondaryLabel
slide            out 160 ease_out_cubic; in 140 ease_in_out_cubic after 220 grace
hover fill       on 80 / off 140 ease_out_cubic
highlight        220 ease_in_out_cubic; rows in 180 / out 140
```
