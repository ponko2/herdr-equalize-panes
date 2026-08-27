# Equalize Panes

[![CI](https://github.com/ponko2/herdr-equalize-panes/actions/workflows/ci.yml/badge.svg)](https://github.com/ponko2/herdr-equalize-panes/actions/workflows/ci.yml)
[![CodeQL](https://github.com/ponko2/herdr-equalize-panes/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/ponko2/herdr-equalize-panes/actions/workflows/github-code-scanning/codeql)

Keeps every pane in a tab the same size as the layout changes.

Splitting a pane in Herdr halves the pane you split,
so a tab drifts towards panes of wildly different sizes.
This plugin rewrites the split ratios after every change,
so the panes go back to sharing the tab evenly.

## Requirements

- Herdr 0.8.2 or later
- ARM64 macOS or 64-bit Linux for a pre-built binary
- Bash
- `curl`, `tar`, and a SHA-256 utility (`shasum` on macOS or `sha256sum` on Linux) for a pre-built binary
- A Rust toolchain with Cargo on other platforms, to build from source

## Install

```sh
herdr plugin install ponko2/herdr-equalize-panes
```

On supported platforms, the installer downloads the matching pre-built binary from GitHub Releases.
On other platforms, it builds from source when Cargo is available.

## Uninstall

```sh
herdr plugin uninstall ponko2.equalize-panes
```

To keep it installed but inactive, disable it instead:

```sh
herdr plugin disable ponko2.equalize-panes
```

`herdr plugin enable ponko2.equalize-panes` brings it back.

## Usage

The plugin runs automatically.
It equalizes the affected tab whenever a pane is created, closed, moved, or exits.

To equalize on demand, use the `Equalize panes` action on a tab.
To reach it from the keyboard, bind the action in `config.toml`:

```toml
[[keys.command]]
key = "prefix+="
type = "plugin_action"
command = "ponko2.equalize-panes.equalize"
description = "Equalize panes"
```

The same action is available from outside Herdr:

```sh
herdr plugin action invoke equalize --plugin ponko2.equalize-panes
```

## How panes are divided

A tab is a binary tree:
every split holds two children and a ratio saying how much of the space the first one gets.
The plugin never touches the tree itself.
Panes keep their positions and their neighbours,
no split changes direction,
and nothing is rearranged into a grid.
Only the ratios change.

The rule is a single line:
every split gets the ratio `panes below its first child / panes below the split`.
Each side then gets the share of the space its pane count deserves,
and applying that at every split leaves each of the `n` panes with `1/n` of the tab.

Splitting the same pane twice leaves one wide pane and two narrow ones,
because the second split only ate into the pane it was made from:

```
┌────────────────┬───────┬───────┐        ┌──────────┬──────────┬──────────┐
│                │       │       │        │          │          │          │
│       A        │   B   │   C   │   ->   │    A     │    B     │    C     │
│                │       │       │        │          │          │          │
└────────────────┴───────┴───────┘        └──────────┴──────────┴──────────┘
       1/2          1/4     1/4               1/3        1/3        1/3
```

The root split has one pane on the left and two on the right,
so its ratio becomes `1/3`.
The split on the right has one pane per side and stays at `1/2`.

Equal means equal area, not equal width and height.
When the splits run in both directions the areas match but the shapes do not:

```
┌────────────────┬───────────────┐        ┌──────────┬─────────────────────┐
│                │       B       │        │          │          B          │
│       A        ├───────────────┤   ->   │    A     ├─────────────────────┤
│                │       C       │        │          │          C          │
└────────────────┴───────────────┘        └──────────┴─────────────────────┘
```

`A` is a tall column while `B` and `C` are wide rows,
but each of them covers a third of the tab.
Panes line up into a grid only if you split them into one.

Two things are left alone.
A zoomed tab is skipped,
because it shows a single pane
and equalizing it would rearrange the layout waiting behind it.
And a split that is already even enough is not touched,
so panes you have resized yourself stay put until the next pane comes or goes.

Terminals are made of whole cells,
so a tab that does not divide evenly is rounded and panes can end up a cell apart.

## How it works

Herdr runs the binary once per trigger.
It reads `HERDR_PLUGIN_EVENT_JSON`, or the action context when invoked as an action,
to decide which tabs to equalize:

| Trigger                      | Tabs equalized                                                  |
| ---------------------------- | --------------------------------------------------------------- |
| `pane.created`               | the tab the pane appeared in                                    |
| `pane.moved`                 | the destination tab, plus the source tab if it still exists     |
| `pane.closed`, `pane.exited` | every tab in the workspace, because the event carries no tab id |
| `Equalize panes` action      | the tab the action was invoked on                               |

A hook can start before the layout catches up with the event that triggered it,
so the plugin re-reads the exported layout
until the pane it expects has appeared or gone.
If it never settles, the last layout it managed to read is equalized anyway.

Ratios are applied to descendants before ancestors,
because resizing a parent first shifts the cell counts its children round to.
Herdr does not serialize plugin hooks,
so each run takes a lock file in `HERDR_PLUGIN_STATE_DIR`
to keep two of them from rewriting the same tree at once.
