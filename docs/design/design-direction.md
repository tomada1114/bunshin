# Bunshin — Design direction

- **Status:** Confirmed 2026-10-02
- **Scope:** a full-screen terminal UI. There is no font, size, spacing scale, or icon set
  to choose — the terminal owns those. What the app decides is which of the terminal's
  own 16 colors and which text modifiers each role uses. Layout and states are in
  [`ux-flows.md`](../product/ux-flows.md); behavior rules in [`ux-guidelines.md`](ux-guidelines.md).
- **The binding lock** — the role table below, recorded as the color lock in the
  `deciding-architecture` skill and carried into code as `const` styles beside the labels
  in the view (`building-tuis`). This file is the research record and the table the lock
  points to; changing a row is a recorded decision there.

## Direction: the terminal's palette, three text colors

Colors are the terminal's 16 ANSI colors, named, never RGB: the owner's theme decides
how each looks, so light and dark terminals are handled by the theme, not by the app.
Text is drawn in only three colors — red, blue, and magenta — because they are the ones
that measured readable on both a dark and a light palette (below); green appears only
on the focused border. Every color doubles a word or a mark that is already there, so
the screen reads the same in a monochrome terminal (`ux-guidelines.md`, Accessibility).

### Primary reference: lazygit's theme model

lazygit styles its interface with ANSI color names plus modifiers, so it follows the
user's theme; by default the focused view's border is green and bold, the other borders
are the terminal's default color, and the selected line has a highlighted background
(<https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md>, `gui.theme`,
checked 2026-10-02).

| | |
|---|---|
| **Preserved** | color by ANSI name, never RGB; the focused region's border green and bold; unfocused borders in the default color |
| **Changed** | the selected row is reversed (default foreground and background swapped) instead of a blue background, so its contrast is the base text's on every theme; the help line's keys are bold rather than blue, because blue means a change here |
| **Rejected** | monochrome with modifiers only (the template's default) — the secretary's messages and the errors would not stand out; a fixed RGB palette — it would ignore the owner's theme and need a light and a dark set |

## Role table

Default means the terminal's own foreground on its own background (ratatui `Color::Reset`).

| Role | Style |
|---|---|
| Base text — task titles, chat text, timestamps, help descriptions | default |
| Focused region: border and title (T1 pane, T2–T5 overlays) | green, bold; heavy border (`BorderType::Thick`) |
| Unfocused region: border and title | default; plain border |
| Selected row in the focused region | reversed |
| Selected row in an unfocused region | bold (the `>` stays) |
| Speaker 「Bunshin」 | magenta, bold |
| Unprompted tag line 「[お知らせ / …]」 | magenta |
| Unprompted tag line 「[質問 / …]」 | magenta, bold |
| Header 「考え中…」「見回り中…」 | magenta |
| Header 「受信箱 N」 (N > 0) | magenta, bold |
| 「── ここから新着 ──」 divider | magenta |
| Speaker 「あなた」 | bold |
| Speaker 「変更」, and the change marks after it | blue |
| Speaker 「システム」 | blue |
| Speaker 「エラー」 | red, bold |
| Error text in a chat row; T2's error line | red |
| Header 「保存できません」「モデル: 使えません」 | red, bold |
| Header 「ミュート中 〜HH:MM」「時間外（…）」「保留 N」 | bold |
| Header app name | bold |
| Leftovers heading in the task pane | bold |
| Help line and T3: the key | bold |
| Dropped task 「[-]」 | crossed out (Terminal.app does not draw it; the mark carries the state) |
| Input length indicator at 400/400 | red |

**Never used:** green, yellow, or cyan for text (they fail on a light palette); bright
black or the faint (DIM) modifier on any text (bright black fails on the owner's theme,
and faint is drawn differently by each terminal); any background color other than
reversed; blinking; RGB or 256-color values.

## Measured contrast

Pairs file: `contrast-pairs.json` (next to this file). Palettes: the owner's Ghostty
1.3.1 `tokyonight` theme, read from the theme file Ghostty ships; Terminal.app's
default colors as the light case (<https://en.wikipedia.org/wiki/ANSI_escape_code>,
"3-bit and 4-bit", Terminal.app column, checked 2026-10-02). Command:
`check_contrast.py contrast-pairs.json` from the owner's `ui-ux-designing` skill (outside
this repository) — exit 0.

```
Name                                                             fg       bg       kind  ratio  required  result
---------------------------------------------------------------  -------  -------  ----  -----  --------  ------
tokyonight (Ghostty, the owner's): base text                     #c0caf5  #1a1b26  text  10.59  4.50      PASS  
tokyonight (Ghostty, the owner's): selected row (reversed)       #1a1b26  #c0caf5  text  10.59  4.50      PASS  
tokyonight (Ghostty, the owner's): Bunshin, tags, 考え中 (magenta)  #bb9af7  #1a1b26  text  7.39   4.50      PASS  
tokyonight (Ghostty, the owner's): 変更, システム (blue)               #7aa2f7  #1a1b26  text  6.79   4.50      PASS  
tokyonight (Ghostty, the owner's): エラー, failing states (red)     #f7768e  #1a1b26  text  6.46   4.50      PASS  
tokyonight (Ghostty, the owner's): focused border (green)        #9ece6a  #1a1b26  ui    9.35   3.00      PASS  
Terminal.app Basic (light): base text                            #000000  #FFFFFF  text  21.00  4.50      PASS  
Terminal.app Basic (light): selected row (reversed)              #FFFFFF  #000000  text  21.00  4.50      PASS  
Terminal.app Basic (light): Bunshin, tags, 考え中 (magenta)         #B200B2  #FFFFFF  text  5.94   4.50      PASS  
Terminal.app Basic (light): 変更, システム (blue)                      #0000B2  #FFFFFF  text  12.78  4.50      PASS  
Terminal.app Basic (light): エラー, failing states (red)            #990000  #FFFFFF  text  8.92   4.50      PASS  
Terminal.app Basic (light): focused border (green)               #00A600  #FFFFFF  ui    3.25   3.00      PASS  
```

Probed and rejected for text on the same two palettes: green 3.25, yellow 3.04, and
cyan 2.96 on Terminal.app's white; bright black 1.91 on tokyonight's background.

## Notes for the architecture

- A palette beyond the terminal's default foreground and background is, by the
  template's `AGENTS.md`, an architecture decision ("a TUI theme beyond the terminal's
  own colors"). This direction is that decision; the owner chose it on 2026-10-02, and it
  is recorded as the color lock in the `deciding-architecture` skill.
- The owner's terminal is Ghostty 1.3.1 with `tokyonight`, `copy-on-select = clipboard`,
  and `macos-option-as-alt = true`: no mouse capture keeps copy-on-select working, and
  Option-based keys arrive as Alt — the key table binds none.
