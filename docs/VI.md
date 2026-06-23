# vi

RustBox includes a small full-screen text editor modeled on classic vi. It assumes a **VT100-compatible terminal**: the applet puts stdin in raw mode and writes ANSI/VT100 escape sequences directly to stdout (clear screen, cursor positioning, status line).

This is **not** a complete vi clone. Only the commands listed below are implemented. Everything else is ignored.

Enable the applet in [`applets.json`](../applets.json) (`"vi": true`, on by default). Applet usage summary: [APPLETS.md](APPLETS.md).

## Usage

```text
vi FILE
vi -T KEYSCRIPT FILE
vi -h
```

| Option | Description |
|--------|-------------|
| `FILE` | File to edit. Created if it does not exist. |
| `-T KEYSCRIPT` | Read keys from a script file instead of the terminal (see [Key scripts](#key-scripts)). |
| `-h`, `--help` | Print usage and exit 0. |

Interactive mode requires **both** stdin and stdout to be terminals. For pipelines, cron, or automated tests, use `-T`.

**Exit status**

| Code | Meaning |
|------|---------|
| `0` | Quit after `:q`, `:wq`, `:q!`, `ZZ`, or `ZQ` |
| `1` | Usage error, I/O failure, scripted session without `:wq`/`:q!`, or `:w`/`:wq` write error |
| `130` | Interrupted (`Ctrl-C`) |

## Modes

| Mode | Enter | Leave |
|------|-------|-------|
| **Normal** | Default; `<Esc>` from other modes | — |
| **Insert** | `i`, `I`, `a`, `A`, `o`, `O` | `<Esc>` |
| **Replace** | `R` | `<Esc>` |
| **Command** | `:` | `<Esc>`, or `<Enter>` after typing a command |
| **Search** | `/` or `?` | `<Esc>`, or `<Enter>` after typing a pattern |

The bottom line shows `-- NORMAL --`, `-- INSERT --`, `-- REPLACE --`, a `:` command line, or a `/` / `?` search prompt.

In insert mode, typed characters are inserted at the cursor. `<Enter>` splits the current line. `<Backspace>` deletes before the cursor or joins with the previous line at column 0.

In replace mode, each typed character overwrites the character under the cursor (without extending the line, except when inserting a newline at end-of-line).

## Count prefix

Digits `1`–`9` in normal mode form a repeat count for the **next** command (default 1). Examples: `3j`, `2x`, `3dd`, `2dw`, `3rX`.

A lone `0` with no pending count moves to the start of the line (column 0). It is **not** treated as a count.

## Motion (normal mode)

| Key | Action |
|-----|--------|
| `h` | Left one character |
| `j` | Down one line |
| `k` | Up one line |
| `l` | Right one character |
| `<Left>`, `<Right>`, `<Up>`, `<Down>` | Same as `h` / `l` / `k` / `j` |
| `0` | Start of line (column 0) |
| `$` | Last character of the line (or column 0 on an empty line) |
| `<End>` | Same as `$` |
| `^` | First non-whitespace character on the line |
| `<Home>` | Same as `^` |
| `G` | Last line of the file |
| `nG` | Go to line `n` (1-based); e.g. `3G` |
| `gg` | First line of the file (type `g` twice) |
| `^f`, `<PageDown>` | Page forward (move down one screen) |
| `^b`, `<PageUp>` | Page back (move up one screen) |
| `^l` | Redraw the screen |

Counts apply to `h`, `j`, `k`, `l`, `^f`, and `^b` (and arrow / page keys). Example: `3l` moves right three columns; `2j` moves down two lines; `2^f` scrolls down two pages.

## Insert (normal mode)

| Key | Action |
|-----|--------|
| `i` | Insert before the cursor |
| `I` | Insert at the beginning of the line |
| `a` | Insert after the cursor |
| `A` | Insert at the end of the line |
| `o` | Open a new line below and enter insert mode |
| `O` | Open a new line above and enter insert mode |

One `u` undoes the entire insert/replace session back to the snapshot taken when insert or replace mode was entered.

## Delete and change (normal mode)

| Key | Action |
|-----|--------|
| `x` | Delete the character under the cursor |
| `dd` | Delete the current line |
| `dw` | Delete from the cursor through the next word |
| `d$` | Delete from the cursor through the end of the line |
| `d^` | Delete from the cursor back to the first non-blank character |
| `cw` | Change from the cursor through the next word (enter insert mode) |
| `cc` | Change the current line (clear it and enter insert mode) |
| `J` | Join the current line with the next (inserts a space when needed) |
| `r` *char* | Replace the character under the cursor with *char* (one `r`, then the replacement character) |
| `R` | Enter replace mode (overwrite until `<Esc>`) |
| `u` | Undo the last change |

**Word** (`dw`): a run of alphanumeric characters, or a single non-alphanumeric character. Deleting at end-of-line joins the next line onto the current one.

`d$` at end-of-line (with nothing left to delete on the current line) joins the next line, like `dw`.

`d^` leaves leading whitespace on the line. On the first non-blank character, it deletes nothing.

Counts apply to `x`, `dd`, `dw`, `cw`, `cc`, and `r`. Example: `2x` deletes two characters; `3dd` deletes three lines.

## Yank and paste (normal mode)

| Key | Action |
|-----|--------|
| `yy` | Yank the current line |
| `Y` | Same as `yy` |
| `yw` | Yank from the cursor through the next word |
| `y$` | Yank from the cursor through the end of the line |
| `p` | Put (paste) after the cursor or below the current line |
| `P` | Put before the cursor or above the current line |

Line yanks (`yy`, `Y`) are pasted as whole lines. Word and end-of-line yanks are pasted as text at the cursor.

Counts apply to `yy`, `yw`, `p`, and `P`.

## Write and quit (normal mode)

| Key | Action |
|-----|--------|
| `ZZ` | Write the buffer to disk and quit |
| `ZQ` | Quit without saving (same as `:q!`) |

## Search (normal mode)

| Key | Action |
|-----|--------|
| `/` *pattern* `<Enter>` | Search forward for literal *pattern* |
| `?` *pattern* `<Enter>` | Search backward for literal *pattern* |
| `n` | Repeat the last search in the same direction |
| `N` | Repeat the last search in the opposite direction |

Search patterns are plain literal strings (not regular expressions). Matches wrap around the file. An empty search prompt repeats the previous pattern, if any.

After `/` or `?`, type the pattern at the bottom of the screen. `<Esc>` cancels without moving. `<Enter>` runs the search and returns to normal mode.

Counts apply to `n` and `N` (for example, `2n` finds the second next match).

## Ex commands (command mode)

Type `:` to open the command line at the bottom of the screen. Press `<Enter>` to run the command, or `<Esc>` to cancel.

| Command | Action |
|---------|--------|
| `:w` | Write the buffer to disk (stay in the editor) |
| `:write` | Same as `:w` |
| `:q` | Quit if the buffer is unchanged |
| `:quit` | Same as `:q` |
| `:q!` | Quit without saving, even if modified |
| `:quit!` | Same as `:q!` |
| `:wq` | Write and quit |
| `:x` | Same as `:wq` |

`:q` on a modified buffer does nothing (the editor stays open). Use `:wq` to save, or `:q!` to discard changes.

Unknown ex commands are ignored.

## Key scripts

`-T KEYSCRIPT` feeds keys from a text file. This is used by the integration tests under [`tests/vi_fixtures/`](../tests/vi_fixtures/) and is the supported way to automate edits without a PTY.

Blank lines in the script are ignored. Literal ASCII characters are sent as-is. Named keys use angle-bracket tokens:

| Token | Sends |
|-------|--------|
| `<Esc>`, `<ESC>` | Escape |
| `<Enter>`, `<CR>` | Enter |
| `<Left>`, `<Right>`, `<Up>`, `<Down>` | Arrow keys |
| `<Home>`, `<End>` | Home / End |
| `<PageUp>`, `<PgUp>` | Same as `^b` |
| `<PageDown>`, `<PgDn>` | Same as `^f` |
| `<Backspace>`, `<BS>` | Backspace |
| `<Delete>`, `<Del>` | Delete |
| `<C-f>`, `<C-b>`, `<C-l>` | `^f`, `^b`, `^l` |
| `<c>` | Single character `c` (any one-character token) |

Example script (`keys.txt`):

```text
A world<Esc>
:wq<Enter>
```

Run:

```bash
rustbox vi -T keys.txt myfile.txt
```

## Display

- Terminal size comes from `TIOCGWINSZ` on stdout (default 80×24) and is rechecked on each input poll (terminal resize).
- The screen is cleared with `\x1b[H\x1b[J` on each redraw.
- The cursor is placed with `\x1b[row;colH` (1-based coordinates) and shown on each redraw (`\x1b[?25h`).
- The cursor is restored on exit if the terminal was left in an unusual state.

## Limitations

Not supported (among many others):

- Visual mode (`v`, `V`)
- Named buffers, macros, `.` repeat
- `:e`, `:set`
- Multi-byte / UTF-8 editing beyond treating UTF-8 as a sequence of Unicode scalar values in the buffer
- Horizontal scrolling of long lines (lines are truncated to terminal width on screen; the full line is still edited in memory)
- Split windows, tags, folds

## Tests

Each supported command has a fixture case under `tests/vi_fixtures/<name>/`:

| File | Purpose |
|------|---------|
| `input.txt` | Starting file contents |
| `keys.txt` | Key script |
| `expected.txt` | Expected file after a successful session |
| `exit` | Optional expected exit code (default `0`) |

Run:

```bash
cargo test --test vi
```
