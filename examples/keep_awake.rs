use screen_wake_lock::ScreenWakeLock;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);

    let reason = args
        .next()
        .unwrap_or_else(|| "Running screen-wake-lock example".to_string());

    let seconds: u64 = args.next().as_deref().unwrap_or("10").parse().unwrap_or(10);

    if !ScreenWakeLock::is_supported() {
        eprintln!("Wake lock is not supported in this environment.");
        std::process::exit(2);
    }

    let _lock = ScreenWakeLock::acquire(&reason)?;

    println!("Keeping the display awake for {seconds}s: {reason}");
    std::thread::sleep(Duration::from_secs(seconds));
    println!("Done (wake lock released when dropped).");

    Ok(())
}
