//! Milestone 1: capture what the system is playing and print its level.

use std::io::Write;
use std::thread;
use std::time::Duration;

use juglar_dsp::{Level, ring, to_dbfs};
use juglar_platform::{AudioCapture, SystemAudio};

/// How often the meter refreshes.
const TICK: Duration = Duration::from_millis(50);
/// Quietest level the meter shows.
const FLOOR_DB: f32 = -60.0;
const METER_WIDTH: usize = 40;

fn main() {
    // One second of mono audio at up to 192 kHz. The main loop drains it every
    // tick, so it never comes close to filling.
    let (mut producer, mut consumer) = ring(192_000);

    let mut mono = Vec::new();
    let capture = SystemAudio::start(Box::new(move |interleaved, channels| {
        downmix(interleaved, usize::from(channels), &mut mono);
        producer.push(&mono);
    }));
    let capture = match capture {
        Ok(capture) => capture,
        Err(e) => {
            eprintln!("juglar: could not capture system audio: {e}");
            std::process::exit(1);
        }
    };

    let info = capture.info();
    println!(
        "Capturing {} Hz, {} channels. Play some music (Ctrl+C to quit).",
        info.sample_rate, info.channels
    );

    let mut block = vec![0.0; info.sample_rate as usize];
    loop {
        thread::sleep(TICK);
        let n = consumer.pop(&mut block);
        print_meter(Level::measure(&block[..n]), consumer.dropped());
    }
}

/// Averages each interleaved frame into one mono sample, reusing `out`.
fn downmix(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    let scale = 1.0 / channels as f32;
    out.extend(interleaved.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() * scale));
}

fn print_meter(level: Level, dropped: usize) {
    let rms_db = to_dbfs(level.rms, FLOOR_DB);
    let peak_db = to_dbfs(level.peak, FLOOR_DB);
    let filled = ((rms_db - FLOOR_DB) / -FLOOR_DB * METER_WIDTH as f32).round() as usize;
    let bar: String = (0..METER_WIDTH).map(|i| if i < filled { '#' } else { '.' }).collect();
    print!("\r[{bar}] rms {rms_db:6.1} dBFS  peak {peak_db:6.1} dBFS  dropped {dropped}   ");
    let _ = std::io::stdout().flush();
}
