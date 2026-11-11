//! Replay: the initial snapshot (which carries `SimConfig`, the seed and data version) plus, per
//! tick, the input and the state hash reached after it. Replaying steps a fresh world from the
//! snapshot and compares every hash.

use crate::SimError;
use crate::codec::{Reader, Writer};
use crate::types::{Command, read_command, write_command};
use crate::world::{SimInput, Snapshot, World};

const MAGIC: &[u8; 8] = b"EDSIMRP1";

#[derive(Clone)]
struct Tick {
    tick: u64,
    utc: f64,
    commands: Vec<Command>,
    hash: u64,
}

pub struct Recorder {
    initial: Snapshot,
    ticks: Vec<Tick>,
}

impl Recorder {
    /// Starts recording from the world's current state.
    pub fn start(world: &World) -> Recorder {
        Recorder { initial: world.snapshot(), ticks: Vec::new() }
    }

    /// Records the input just stepped and the hash the world reached after it.
    pub fn record(&mut self, input: &SimInput, hash_after: u64) {
        self.ticks.push(Tick { tick: input.tick, utc: input.utc, commands: input.commands.to_vec(), hash: hash_after });
    }

    pub fn finish(self) -> Vec<u8> {
        let mut w = Writer::default();
        w.buf.extend_from_slice(MAGIC);
        self.initial.write(&mut w);
        w.len(self.ticks.len());
        for t in &self.ticks {
            w.u64(t.tick);
            w.f64(t.utc);
            w.len(t.commands.len());
            for c in &t.commands {
                write_command(&mut w, c);
            }
            w.u64(t.hash);
        }
        w.buf
    }
}

/// The first tick whose hash differs from the recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub tick: u64,
    pub expected: u64,
    pub actual: u64,
}

pub struct Replay {
    initial: Snapshot,
    ticks: Vec<Tick>,
}

impl Replay {
    pub fn from_bytes(data: &[u8]) -> Result<Replay, SimError> {
        let mut r = Reader::new(data);
        for &m in MAGIC {
            if r.u8()? != m {
                return Err(SimError::Corrupt("not a replay"));
            }
        }
        let initial = Snapshot::read(&mut r)?;
        let n = r.len(28)?;
        let mut ticks = Vec::with_capacity(n);
        for _ in 0..n {
            let tick = r.u64()?;
            let utc = r.f64()?;
            let nc = r.len(9)?;
            let mut commands = Vec::with_capacity(nc);
            for _ in 0..nc {
                commands.push(read_command(&mut r)?);
            }
            let hash = r.u64()?;
            ticks.push(Tick { tick, utc, commands, hash });
        }
        if !r.done() {
            return Err(SimError::Corrupt("trailing bytes"));
        }
        Ok(Replay { initial, ticks })
    }

    pub fn len(&self) -> usize {
        self.ticks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ticks.is_empty()
    }

    pub fn initial(&self) -> &Snapshot {
        &self.initial
    }

    /// The recorded per-tick hashes.
    pub fn hashes(&self) -> Vec<u64> {
        self.ticks.iter().map(|t| t.hash).collect()
    }

    /// Steps a fresh world from the initial snapshot through every recorded tick. Returns the
    /// per-tick hashes, or the first tick that differs from the recording.
    pub fn run(&self) -> Result<Result<Vec<u64>, Divergence>, SimError> {
        let mut world = World::from_snapshot(&self.initial)?;
        let mut out = Vec::with_capacity(self.ticks.len());
        for t in &self.ticks {
            world.step(&SimInput { tick: t.tick, commands: &t.commands, utc: t.utc });
            let h = world.state_hash();
            if h != t.hash {
                return Ok(Err(Divergence { tick: t.tick, expected: t.hash, actual: h }));
            }
            out.push(h);
        }
        Ok(Ok(out))
    }
}
