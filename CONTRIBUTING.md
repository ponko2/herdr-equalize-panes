# Contributing

How to build, test, and run the plugin from a working tree.
The [README](README.md) covers what the plugin does and how it divides panes.

## Prerequisites

- A Rust toolchain.
  `rust-toolchain.toml` pins the version,
  so `rustup` fetches the right one on the first `cargo` command.
- Herdr 0.8.2 or later, to run what you build.
- [hk](https://hk.jdx.dev), for the git hooks.

## Development

```sh
cargo test
cargo clippy --all-targets
cargo fmt
```

Lints are configured in `Cargo.toml` rather than passed on the command line,
so these commands and the editor agree.
`hk check` runs clippy and rustfmt through `hk.pkl`,
and the pre-commit hook runs them on what you staged.
Neither runs the tests, so run `cargo test` yourself.

`make help` lists the build and install targets.

## Debug and release builds

`herdr-plugin.toml` points every action and event hook at `./target/release/equalize-panes`.
While the plugin is linked:

- `cargo test` and `cargo build` only touch `target/debug/`,
  and Herdr never runs those.
  Test as much as you like.
- `cargo build --release` replaces the binary Herdr actually runs.
  From that moment the plugin acts on real pane events.

Use `herdr plugin disable ponko2.equalize-panes` to stop it without unlinking.

## Linking and unlinking

```sh
make install      # cargo build --release, then herdr plugin link $(CURDIR)
make uninstall    # herdr plugin unlink ponko2.equalize-panes
herdr plugin list
```

`herdr plugin link` does not run `[[build]]` commands —
those run only for `herdr plugin install` from GitHub.
Local authors build their own working tree, which is why `make install` builds first.

Linking writes `~/.config/herdr/plugins.json`,
and that snapshot goes stale as soon as you edit the manifest.
The live state is whatever the server loaded: trust `herdr plugin action list`.

## Inspecting live Herdr state

The socket API is newline-delimited JSON:
one request per line, one response, then the server closes the connection.
Reads are safe to run against your own session.

```sh
printf '%s\n' '{"id":"x","method":"layout.export","params":{"tab_id":"'"$HERDR_TAB_ID"'"}}' \
  | nc -U "$HERDR_SOCKET_PATH"

printf '%s\n' '{"id":"x","method":"tab.list","params":{}}' | nc -U "$HERDR_SOCKET_PATH"
```

Capture real payloads this way and pin them as fixtures,
as `src/herdr/socket.rs` does.
`herdr api schema --output schema.json` dumps the full request, response, and event schemas.

## Testing against a fake server

The plugin's entire interface is environment variables,
so you can exercise a release binary end to end without touching your session:
point `HERDR_SOCKET_PATH` at a socket you control and answer the requests yourself.

```sh
env HERDR_SOCKET_PATH=/tmp/fake.sock \
    HERDR_PLUGIN_STATE_DIR=/tmp/fake-state \
    HERDR_TAB_ID=w1:t1 \
    HERDR_PLUGIN_LOG=debug \
    ./target/release/equalize-panes
```

This is the only check that covers `plugin.rs`, which unit tests do not reach.
Keep the socket path short:
macOS caps Unix socket paths at 104 bytes,
and a path under a deep temporary directory will not fit.

## Reading plugin logs

Herdr records each run's argv, exit status, stdout, and stderr.

```sh
herdr plugin log list --plugin ponko2.equalize-panes --limit 5
```

That is the only window into a hook,
since you cannot attach a debugger to a process Herdr spawns.
`HERDR_PLUGIN_LOG` sets the level and defaults to `warn`,
so ordinary runs stay silent.
A hook inherits the Herdr server's environment, not your shell's,
so raising the level for hooks means starting the server with the variable set.
Running the binary by hand picks it up from your shell.
