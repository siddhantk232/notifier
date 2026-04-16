# notify

A macOS notification CLI built in Rust. Sends native notifications with automatic tmux session/window context and supports scheduled reminders via launchd.

## Usage

### Send a notification

```sh
notify send "Build done"
notify send "Deployed" --title "deploy"
```

When run inside tmux, the notification subtitle automatically shows the current session and window (e.g. `tmux: dev:editor`).

### Chain with commands

```sh
cargo build && notify send "Build succeeded"
cargo build || notify send "Build failed"
make test; notify send "Tests finished"
```

### Schedule reminders

Reminders are managed as launchd agents (`~/Library/LaunchAgents/`). Schedule uses cron syntax converted to `StartCalendarInterval`.

```sh
# Weekdays at 9am
notify remind "standup" --cron "0 9 * * 1-5"

# Every day at noon
notify remind "lunch break" --cron "0 12 * * *"

# One-shot reminder (auto-removes after firing)
notify remind "deploy to prod" --cron "30 14 16 4 *" --once
```

### List and remove reminders

```sh
notify list
# ID         SCHEDULE             MESSAGE
# a1b2c3d4   0 9 * * 1,2,3,4,5   standup

notify remove a1b2c3d4
```

## Install

```sh
cargo install --path .
```

> **Note:** If your default `cc` is GCC (e.g. on NixOS), you need clang for the native macOS bindings:
> ```sh
> CC=/usr/bin/clang cargo install --path .
> ```
> The repo includes `.cargo/config.toml` that sets clang as the linker, but `CC` is still needed for the `mac-notification-sys` build script.

## How it works

- **Notifications** via [`mac-notification-sys`](https://crates.io/crates/mac-notification-sys) — native Objective-C bindings to macOS notification APIs, no AppleScript
- **Tmux context** detected from `$TMUX` env var and `tmux display-message` — shown as the notification subtitle
- **Reminders** via launchd plist files in `~/Library/LaunchAgents/` — the standard macOS scheduler. Cron expressions are parsed and converted to `StartCalendarInterval` dicts

## Cron expression support

Standard 5-field cron: `minute hour day-of-month month day-of-week`

| Expression | Meaning |
|---|---|
| `0 9 * * *` | Every day at 9:00 AM |
| `0 9 * * 1-5` | Weekdays at 9:00 AM |
| `30 17 * * 5` | Fridays at 5:30 PM |
| `0 0 1 * *` | First of every month at midnight |

Step values (`*/5`) are not supported since launchd's `StartCalendarInterval` doesn't have an equivalent. Use specific values instead.

## Requirements

- macOS
- Rust toolchain
- clang (ships with Xcode Command Line Tools)

---

*This project was entirely created by [Claude](https://claude.ai) (Anthropic's AI assistant) using [Claude Code](https://claude.com/claude-code).*
