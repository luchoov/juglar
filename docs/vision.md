# Vision

> **Juglar is a music visualizer that listens like a musician.**
> It understands a song's structure, harmony and anticipation, not just its loudness. Every song gets its own visual identity, and when the chorus comes back, so does its image.

## The gap

Audio-reactive visuals are old. MilkDrop and projectM have rendered them on the GPU for twenty years; feedback loops, palettes, bloom and even randomly mixed presets (MilkDrop 2's mash-ups) are known techniques. Being light and written in Rust is a quality, not an idea.

What almost every visualizer shares is that it **reacts frame by frame**: loudness and spectrum *right now*, with no memory and no musical understanding. It listens like a level meter. A human VJ does something different: they know the drop is coming, they bring back the chorus look when the chorus returns, they color a minor passage differently from a major one.

Juglar's innovation is to give the generator that kind of listening.

## The five ideas

### 1. Song structure: visual memory

Detect, live, that a section of the song **repeats**, by comparing each bar with every previous one (a self-similarity matrix over beat-synchronous features). When the chorus returns, its visual motif returns too, perhaps evolved. The song's form becomes visible, like a leitmotif.

### 2. A visual identity per song

Derive the generator's seed from an audio fingerprint. **The same song always builds the same visual world**; a different song builds another. Generative, but not arbitrary: over time you could recognize a song by how it looks.

### 3. Harmony as color

Most visualizers ignore harmony entirely. A **chromagram** (energy per pitch class, folded from the same FFT) gives the notes present; from it, an estimate of key and chords. Major/minor, tension/resolution and key changes drive the palette. Historical lineage: Scriabin's *Prometheus* (1910) includes a part for a keyboard of light.

### 4. Rhythm moves, melody paints

**Harmonic/percussive separation** (median filtering of the spectrogram along time and frequency) splits the signal cheaply: percussive energy drives geometry and motion, harmonic energy drives color and shape. Each layer of the music controls its own dimension of the image.

### 5. Anticipation: tension and release

Track the tempo to **predict** where the next beat lands instead of reacting late. Detect build-ups (rising energy, risers, increasing onset density) to accumulate visual tension that releases exactly on the drop.

## Constraints these ideas must respect

- **Still nearly free on the CPU.** Chroma and harmonic/percussive separation reuse the FFT already computed; structure analysis works on a few hundred feature vectors per song.
- **Just more `Signals`.** Section, recurrence, key, tension and predicted beat are new fields of the abstract input the generator already consumes. Nothing in the architecture changes.
- **Honest.** Focus/relaxation modes are style options, not claimed benefits (see `docs/research/focus-and-relaxation.md`). Flash and pattern limits apply in every mode.

## Explorables

Every interesting technique gets an **explorable explanation**: an interactive page in `explorables/` where you move a control and watch the algorithm respond, next to the actual code that does it. They exist to learn from, to show people, and eventually to live on juglar.dev. See `explorables/README.md`.
