//! Backend-independent resource-state tracking and validation (contract invariant 2).
//!
//! Every backend keeps one of these per resource. Metal only uses it to report caller
//! mistakes; Vulkan and D3D12 also need the tracked state to emit barriers.

use crate::{ResourceState, Subresource, Usage};

/// Whether `usage` allows a resource to be put in `state`.
pub(crate) fn usage_allows(usage: &Usage, state: ResourceState, presentable: bool) -> bool {
    use ResourceState::*;
    match state {
        Undefined => true,
        ShaderRead => usage.shader_read || usage.depth,
        ShaderWrite => usage.shader_write,
        RenderTarget => usage.render_target,
        DepthWrite | DepthRead => usage.depth,
        IndirectArgs => usage.indirect_args,
        CopySrc => usage.copy_src,
        CopyDst => usage.copy_dst,
        Present => presentable,
    }
}

/// State of every subresource (mip x layer) of one texture, or of one buffer (1 x 1).
pub(crate) struct States {
    mips: u32,
    layers: u32,
    states: Vec<ResourceState>,
}

impl States {
    pub fn new(mips: u32, layers: u32) -> Self {
        Self { mips, layers, states: vec![ResourceState::Undefined; (mips * layers) as usize] }
    }

    fn range(&self, sub: Option<Subresource>) -> Result<std::ops::Range<usize>, String> {
        match sub {
            None => Ok(0..self.states.len()),
            Some(s) if s.mip < self.mips && s.layer < self.layers => {
                let i = (s.layer * self.mips + s.mip) as usize;
                Ok(i..i + 1)
            }
            Some(s) => Err(format!("subresource mip {} layer {} out of range ({} mips, {} layers)", s.mip, s.layer, self.mips, self.layers)),
        }
    }

    /// Applies a declared transition. `from` must be the tracked state, or `Undefined`
    /// (discard).
    pub fn transition(&mut self, sub: Option<Subresource>, from: ResourceState, to: ResourceState) -> Result<(), String> {
        let r = self.range(sub)?;
        if from != ResourceState::Undefined
            && let Some(bad) = self.states[r.clone()].iter().find(|s| **s != from)
        {
            return Err(format!("transition from {from:?}, but the resource is in {bad:?}"));
        }
        for s in &mut self.states[r] {
            *s = to;
        }
        Ok(())
    }

    /// The subresource(s) must all be in one of `allowed`.
    pub fn expect(&self, sub: Option<Subresource>, allowed: &[ResourceState]) -> Result<(), String> {
        let r = self.range(sub)?;
        match self.states[r].iter().find(|s| !allowed.contains(s)) {
            Some(bad) => Err(format!("needs {allowed:?}, but is in {bad:?}")),
            None => Ok(()),
        }
    }

    /// Tracked state of one subresource (`None` for out-of-range).
    #[allow(dead_code)]
    pub fn get(&self, mip: u32, layer: u32) -> Option<ResourceState> {
        if mip < self.mips && layer < self.layers { Some(self.states[(layer * self.mips + mip) as usize]) } else { None }
    }

    #[allow(dead_code)]
    pub fn mips(&self) -> u32 {
        self.mips
    }

    #[allow(dead_code)]
    pub fn layers(&self) -> u32 {
        self.layers
    }

    /// Sets without checking (backend-internal moves such as an upload into `Undefined`).
    pub fn set(&mut self, sub: Option<Subresource>, to: ResourceState) {
        if let Ok(r) = self.range(sub) {
            for s in &mut self.states[r] {
                *s = to;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceState::*;

    #[test]
    fn transitions_check_source() {
        let mut s = States::new(2, 1);
        s.transition(None, Undefined, CopyDst).unwrap();
        assert!(s.transition(None, ShaderRead, RenderTarget).is_err());
        s.transition(Some(Subresource { mip: 1, layer: 0 }), CopyDst, ShaderRead).unwrap();
        assert!(s.expect(None, &[CopyDst]).is_err());
        s.expect(Some(Subresource { mip: 0, layer: 0 }), &[CopyDst]).unwrap();
        assert!(s.expect(Some(Subresource { mip: 2, layer: 0 }), &[CopyDst]).is_err());
    }

    #[test]
    fn usage_gates_states() {
        let u = Usage { shader_read: true, ..Default::default() };
        assert!(usage_allows(&u, ShaderRead, false));
        assert!(!usage_allows(&u, RenderTarget, false));
        assert!(!usage_allows(&u, Present, false));
    }
}
