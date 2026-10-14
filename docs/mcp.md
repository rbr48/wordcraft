# MCP server

WordCraft speaks the [Model Context Protocol](https://modelcontextprotocol.io) over stdio, so
Claude and other agents can write and edit documents.

```sh
wordcraft-cli mcp                          # headless session (no window)
wordcraft --control 7981 &                 # or: drive the running app…
wordcraft-cli mcp --connect 127.0.0.1:7981 # …including clicks, keys and screenshots
```

Claude Code: `claude mcp add wordcraft -- wordcraft-cli mcp`.

With `--connect`, the bridge sends the app's control key, which it reads from
`WORDCRAFT_CONTROL_KEY` or from the key file the app writes at start. It sends the key only to
loopback addresses; see [Keys](control-protocol.md#keys).

## Tracked changes by an agent

Tracked changes and comments are recorded under the session's user name ("WordCraft User" by
default). Name the agent so reviewers can tell its edits from people's: start the headless server
with `wordcraft-cli mcp --author "Claude (copyedit)"`, or run
`file.setAuthor {"name": "Claude (copyedit)"}`. Turn tracking on explicitly with
`review.trackChanges {"value": true}`; without `value` it toggles, which turns tracking *off* in
a document that was saved with it on. `wordcraft-cli run` takes `--author` too.

## Tools

| Tool | What it does |
|---|---|
| `list_commands` | every command (filter with `query`) — over 270 of them |
| `execute` | run one command: `{command: "insert.table", params: {rows: 3, cols: 4}}` |
| `batch` | run several commands in order |
| `new_document` | blank, sample, letter, resume, report |
| `open_document` / `save_document` | docx, doc (open only), odt, rtf, html, md, tex, txt, json; save also pdf, png |
| `type_text` | type at the caret (`paragraphs: true` splits lines into paragraphs) |
| `select_text` | select the n-th occurrence of some text |
| `get_text` / `inspect_document` | read the document back (verify without screenshots) |
| `render_page` | a page as PNG |
| `parity` | feature coverage |
| `screenshot`, `click`, `key`, `ui_inspect` | app only (`--connect`) |

Resources: `wordcraft://document` (inspect) and `wordcraft://commands`.

The acceptance test `crates/mcp/src/tests.rs` writes a formatted document using MCP only.

## Character and paragraph properties

`format.set {"props": CharProps}`, `para.set {"props": ParaProps}`,
`styles.modify {"style": …, "chr"?: CharProps, "para"?: ParaProps}` and the `para` of
`table.newStyle` / `table.modifyStyle` take these two objects. Every field is optional: a field
left out keeps what the style chain gives. A field name the object doesn't know is ignored, not
refused, so check the spelling. Lengths are points; `[r, g, b]` is a colour as three numbers
0–255.

```json
{"command": "styles.modify", "params": {"style": "Heading1",
  "chr": {"font": "Georgia", "size": 18, "bold": true, "color": {"Rgb": [27, 22, 17]}},
  "para": {"spaceBefore": 12, "lineSpacing": {"rule": "multiple", "value": 1.15}, "keepNext": true}}}
```

### CharProps

| Field | Type | Values |
|---|---|---|
| `style` | string | character style id |
| `font` | string | font name |
| `size` | number | points |
| `bold`, `italic`, `strike`, `doubleStrike`, `caps`, `smallCaps`, `hidden`, `outline`, `shadow`, `emboss`, `engrave`, `noProof` | bool | |
| `underline` | string | `none`, `single`, `words`, `double`, `thick`, `dotted`, `dash`, `dotDash`, `dotDotDash`, `wave`, `doubleWave` |
| `underlineColor` | `[r, g, b]` | |
| `color` | `"Auto"`, a hex string `"RRGGBB"` or `"#RRGGBB"`, or `{"Rgb": [r, g, b]}` | `Auto` (also `"auto"`) is black on light backgrounds, white on dark; read back as `"Auto"` or `{"Rgb": …}` (#865) |
| `highlight` | string | `none`, `yellow`, `brightGreen`, `turquoise`, `pink`, `blue`, `red`, `darkBlue`, `teal`, `green`, `violet`, `darkRed`, `darkYellow`, `gray50`, `gray25`, `black`, `white` |
| `shading` | `[r, g, b]` | background fill |
| `border` | Border | character border; `"style": "none"` is explicitly no border |
| `vertAlign` | string | `baseline`, `superscript`, `subscript` |
| `spacing` | number | character spacing, points: positive expands, negative condenses |
| `scale` | number | horizontal scale, percent (100 = normal) |
| `position` | number | raised (positive) or lowered (negative), points |
| `kern` | number | kern fonts at or above this size, points; 0 = off |
| `textEffects` | object | WordArt `outline`, `shadow`, `fill` (a gradient), and `warp` (a preset from the DrawingML names, with an `amount` from 0 to 1) (see `crates/doc/src/text_effects.rs`) |
| `lang` | string | BCP 47 language tag (proofing) |
| `rtl` | bool | right-to-left run |
| `cs` | bool | format the whole run with the complex-script properties |
| `fontCs`, `fontEa` | string | complex-script font; East Asian font |
| `sizeCs` | number | complex-script size, points |
| `boldCs`, `italicCs` | bool | complex-script bold, italic |
| `langBidi` | string | BCP 47 language of complex-script text, e.g. `fa-IR` |
| `link` | string | hyperlink target: a URL, or `#bookmark` |
| `ins`, `del`, `fmtChange` | | Track Changes bookkeeping (revision indexes) |

`format.set` also applies `size`, `bold` and `italic` to `sizeCs`, `boldCs` and `italicCs` when
those aren't given; `styles.modify` sets only what it is given.

A **Border** is `{"style": …, "width": pt, "color": [r, g, b] | null, "space": pt}` (`space` is
the distance from the text). Missing fields are `none`, 0, no colour and 0, so a border needs a
`width` above 0 to show. `style` is one of `none`, `single`, `double`, `dotted`, `dashed`,
`thick`, `triple`, `dotDash`, `wave`.

### ParaProps

| Field | Type | Values |
|---|---|---|
| `style` | string | paragraph style id |
| `align` | string | `left`, `center`, `right`, `justify`, `distribute` (`left` is the start edge: the right in a right-to-left paragraph) |
| `indentLeft`, `indentRight` | number | points |
| `indentFirst` | number | first-line indent, points; negative = hanging |
| `indentLeftChars`, `indentRightChars`, `indentFirstChars` | number | the same indents in characters; a non-zero value wins over the one in points, 0 cancels an inherited one |
| `spaceBefore`, `spaceAfter` | number | points |
| `lineSpacing` | object | `{"rule": "multiple", "value": 1.15}` (1 = single, 2 = double), `{"rule": "atLeast", "value": pt}`, `{"rule": "exactly", "value": pt}` |
| `contextualSpacing` | bool | ignore space before/after between paragraphs of the same style |
| `mirrorIndents` | bool | left and right indents swap on even pages |
| `keepNext`, `keepLines`, `pageBreakBefore`, `widowControl` | bool | |
| `outlineLevel` | integer | 0–8 = Level 1–9, 9 = body text |
| `numbering` | object | `{"num": n, "level": n}`, both required; `level` counts from 0; `num` 0 removes numbering a style gives |
| `tabs` | array | `[{"pos": pt, "align"?: "left\|center\|right\|decimal\|bar\|clear", "leader"?: "none\|dot\|hyphen\|underscore\|middleDot"}]`; replaces the inherited stops, `clear` removes an inherited one at `pos` |
| `shading` | `[r, g, b]` | background fill |
| `borders` | object | `{"top", "left", "bottom", "right", "between"}`, each a Border (`between`: between paragraphs; `inside_v` is used by tables) |
| `suppressHyphens`, `suppressLineNumbers` | bool | |
| `bidi` | bool | right-to-left paragraph |
| `dropCap` | integer | lines to drop, 0 = none |
| `kinsoku`, `wordWrap`, `overflowPunct`, `autoSpaceDE`, `autoSpaceDN`, `snapToGrid` | bool | East Asian typography; unset = on |
| `topLinePunct` | bool | East Asian typography; unset = off |
| `numChange`, `fmtChange` | | Track Changes bookkeeping |
