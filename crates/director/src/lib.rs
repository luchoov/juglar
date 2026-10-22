//! The director: decides what Juglar shows and when.
//!
//! The renderer draws whatever scene it is told to, as lively as it is told
//! to. This crate makes those calls from the music, the way a VJ would:
//! how lively to be (`intensity`), from slow summaries of the signals
//! (`feel`). Plain Rust with no OS calls, like the signals it reads, so it
//! also builds for `wasm32-unknown-unknown`.

pub mod feel;
pub mod intensity;
pub mod transition;

pub use feel::{Feel, Listener};
pub use intensity::NaturalIntensity;
pub use transition::{Style, Transition};

use juglar_signals::Signals;

/// Who sets the intensity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intensity {
    /// Read from the music (`intensity.rs`).
    Natural,
    /// Fixed by the listener, `0..=1`.
    Fixed(f32),
}

pub struct Director {
    listener: Listener,
    natural: NaturalIntensity,
    intensity: Intensity,
}

impl Director {
    pub fn new(intensity: Intensity) -> Self {
        let start = match intensity {
            Intensity::Natural => 0.3,
            Intensity::Fixed(value) => value,
        };
        Self { listener: Listener::new(), natural: NaturalIntensity::new(start), intensity }
    }

    pub fn intensity(&self) -> Intensity {
        self.intensity
    }

    /// Switches who sets the intensity. Natural mode starts from the
    /// current value, so switching to it changes nothing at once.
    pub fn set_intensity(&mut self, intensity: Intensity, current: f32) {
        if intensity == Intensity::Natural && self.intensity != Intensity::Natural {
            self.natural = NaturalIntensity::new(current);
        }
        self.intensity = intensity;
    }

    /// What the music has felt like lately.
    pub fn feel(&self) -> Feel {
        self.listener.feel()
    }

    /// Listens to this frame's signals and writes the decisions into them.
    pub fn update(&mut self, s: &mut Signals) {
        let feel = self.listener.update(s);
        let natural = self.natural.update(&feel, s.dt);
        s.intensity = match self.intensity {
            Intensity::Natural => natural,
            Intensity::Fixed(value) => value,
        };
    }
}
