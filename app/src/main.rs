//! Milestone 2: capture what the system is playing and show its bands and
//! spectrum live in the terminal.

use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use juglar_dsp::{Analysis, Analyzer, ring};
use juglar_platform::{AudioCapture, SystemAudio};

/// How often the display refreshes; about 30 frames per second.
const TICK: Duration = Duration::from_millis(33);
const METER_WIDTH: usize = 10;
/// Eight heights of block, from empty to full.
const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

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

    let mut analyzer = Analyzer::new(info.sample_rate);
    let mut block = vec![0.0; info.sample_rate as usize];
    let mut last = Instant::now();
    loop {
        thread::sleep(TICK);
        let now = Instant::now();
        let dt = (now - last).as_secs_f32();
        last = now;

        let n = consumer.pop(&mut block);
        analyzer.push(&block[..n]);
        print_frame(analyzer.analyze(dt));
    }
}

/// Averages each interleaved frame into one mono sample, reusing `out`.
fn downmix(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    let scale = 1.0 / channels as f32;
    out.extend(interleaved.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() * scale));
}

fn print_frame(a: &Analysis) {
    let spectrum: String = a.spectrum.iter().map(|&v| block_char(v)).collect();
    print!(
        "\rbass {} mid {} treble {} energy {} |{spectrum}| ",
        meter(a.bass),
        meter(a.mid),
        meter(a.treble),
        meter(a.energy),
    );
    let _ = std::io::stdout().flush();
}

fn meter(value: f32) -> String {
    let filled = (value * METER_WIDTH as f32).round() as usize;
    (0..METER_WIDTH).map(|i| if i < filled { '█' } else { '·' }).collect()
}

fn block_char(value: f32) -> char {
    BLOCKS[((value * 8.0).round() as usize).min(8)]
}
