# `screen-wake-lock`

Minimal cross-platform screen wake lock library.

## Overview

This library provides a simple guard that prevents the display from entering idle/sleep mode while it remains alive. It supports Windows, macOS, and Linux.

## Usage

```rust
use screen_wake_lock::ScreenWakeLock;

// Acquire a wake lock
let lock = ScreenWakeLock::acquire("Playing video")?;

// Keep running...
// The screen will stay awake while the lock is alive

// Release the lock (also happens automatically when dropped)
drop(lock);
```

## Platform Support

- **Windows**: Uses `SetThreadExecutionState` API
- **macOS**: Uses IOKit power management
- **Linux**: Uses D-Bus inhibitor service (requires a session bus)

## Additional Features

- Check if wake lock is supported: `ScreenWakeLock::is_supported()`
- Linux-specific options: `ScreenWakeLock::acquire_with_linux_options()`

## Example

```bash
cargo run --example keep_awake "Watching movie" 30
```
