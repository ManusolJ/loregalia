# Design

Dark first, with a light theme one setting away. A new view should be buildable from this page
without opening another view to copy values.

## Color tokens

Values live in `crates/app/src/theme.rs` and nowhere else. Its tests enforce the contrast
requirements below. Tokens are named by role, never by color.

| Token                       | Role                                                    | Contrast needed |
| --------------------------- | ------------------------------------------------------- | --------------- |
| `bg`                        | Window background                                       | -               |
| `surface`                   | Sidebar, cards, panels, inputs                          | -               |
| `raised`                    | Menus, popovers, dialogs, drawers                       | -               |
| `content`                   | Body text, titles                                       | 7:1             |
| `muted`                     | Secondary text: year, runtime, counts, placeholders     | 4.5:1           |
| `line`                      | Dividers, card edges; decorative only                   | -               |
| `line_strong`               | Outlines of controls: inputs, checkboxes                | 3:1             |
| `accent`                    | Links, primary buttons, focus outline, selection        | 4.5:1           |
| `on_accent`                 | Text and icons on an `accent` fill                      | 4.5:1           |
| `danger`                    | Destructive actions, error text                         | 4.5:1           |
| `status_*`                  | Status category colors                                  | 3:1             |

Elevation is a lighter surface: `bg`, then `surface`, then `raised`.

## Type

The system font (D-13). Nothing goes below 12 px. Use three weights only: 400, 500, 600.

| Role                                 | Size / line height | Weight             |
| ------------------------------------ | ------------------ | ------------------ |
| Metadata: year, runtime, chip label  | 12 / 16            | 500                |
| UI default: rows, buttons, inputs    | 14 / 20            | 400; 500 on buttons |
| Reading: synopsis, descriptions      | 16 / 24            | 400                |
| Section headings, detail title       | 18 / 28            | 600                |
| Page title                           | 24 / 32            | 600                |

Reading text is capped at about 65 characters wide.

## Spacing and density

Compact, because a planner is long lists. Use only these steps: 4, 8, 12, 16, 24, 32 px.

| Element                 | Spacing                         |
| ----------------------- | ------------------------------- |
| Control (button, input) | 32 px tall, 12 px side padding  |
| Icon-only button        | 32 x 32 px                      |
| List row                | 12 / 8 px padding, 12 px gap    |
| Card or panel           | 16 px padding                   |
| Between sections        | 24 px                           |
| Page gutter             | 24 px                           |

Nothing clickable is smaller than 24 x 24 px.

## Radius and borders

- 6 px: buttons, inputs, chips, covers.
- 8 px: cards, panels, menus, dialogs.
- Borders are 1 px: `line` on cards and dividers, `line_strong` on controls.

## Window and layout

The window opens at 1280 x 800, with a 960 x 600 minimum. There are three regions: sidebar, main and
detail.

| Window width | Sidebar             | Detail                    |
| ------------ | ------------------- | ------------------------- |
| 1280 and up  | 240 px, with labels | Docked on the right, 360 px |
| 1024-1279    | 240 px, with labels | Drawer over the main area |
| 960-1023     | 56 px icon rail     | Drawer over the main area |

## Covers

- One ratio: 2:3, cropped and centered (`ObjectFit::Cover`).
- No image: a `surface` tile with the media-kind icon in `muted`, and the title clamped to three
  lines. Never show a broken image.
- Grid tiles are at least 144 px wide; row thumbnails are 32 x 48 px.
- 6 px radius with a `line` edge, so a dark poster does not dissolve into the background.
- Off-screen covers should not load. Use a virtual list for long grids and lists.

## States

| State    | Looks like                                                          |
| -------- | ------------------------------------------------------------------- |
| Hover    | Rows and ghost buttons: `content` at 5%. Filled buttons: `accent` at 90% |
| Selected | `accent` at 15%; text stays `content`                               |
| Focus    | 2 px `accent` outline, offset 2 px. Never removed without a replacement |
| Disabled | 50% opacity, no hover change                                        |
| Invalid  | `danger` border, plus a 12 px `danger` message; never color alone   |
| Loading  | Blocks of `content` at 5%                                           |

Color transitions take 150 ms. Nothing else moves on its own.

## Statuses

The set of statuses can grow, so color comes from a status's category, not the status itself. A new
status picks a category and inherits its color and icon (D-04).

| Category  | Examples                   | Token            | Lucide icon     |
| --------- | -------------------------- | ---------------- | --------------- |
| `planned` | To watch, To read, To play | `status_planned` | `circle-dashed` |
| `active`  | In progress                | `status_active`  | `circle-play`   |
| `paused`  | On hold                    | `status_paused`  | `circle-pause`  |
| `done`    | Watched, Read, Played      | `status_done`    | `circle-check`  |
| `dropped` | Dropped                    | `status_dropped` | `circle-x`      |

A status chip is the icon plus the label, never a colored dot alone: 24 px tall, the category color
at 15% behind it, the icon in the category color, and the label in `content`.

Media kinds (movie, show, book, game) get the icons `film`, `tv`, `book-open` and `gamepad-2` in
`muted`, with no color of their own.

## Icons

Lucide, through `gpui-kit`'s bundled assets, so they work offline. 16 px next to body text, 20 px in
the sidebar.

## Theme

The setting offers Dark (the default), Light and System. Apply a choice with `theme::apply`, which
also updates `gpui-kit`'s component colors. The choice is stored in the `settings` table (D-14).
