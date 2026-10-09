//! Deterministic bounded preparation of tensor identity, including cold storage.
use super::*;

/// Inputs remain immutable shared pages. Traversal never skips a cached subtree:
/// cache warmth must not change task handoff points or recorded replay.
#[derive(Clone)]
pub struct TensorIdentityWork {
    array: Array,
    pending: Vec<(usize, u8)>,
    hashes: Vec<[u8; 32]>,
    result: Option<[u8; 32]>,
    trusted_prefix: bool,
}

impl TensorIdentityWork {
    pub const STEP_WORDS: usize = 4096;

    pub fn new(array: &Array) -> Result<Self> {
        if array.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        Ok(Self {
            array: array.clone(),
            pending: vec![(1, 0)],
            hashes: Vec::new(),
            result: None,
            trusted_prefix: true,
        })
    }

    fn node(&self, path: usize) -> &Node {
        let mut node = self.array.buffer.root.as_ref();
        let depth = usize::BITS - path.leading_zeros() - 1;
        for bit in (0..depth).rev() {
            let NodeKind::Branch(left, right) = &node.kind else {
                unreachable!("internal traversal path");
            };
            node = if path & (1 << bit) == 0 { left } else { right };
        }
        node
    }

    /// Returns charged scalar/node units; never exceeds STEP_WORDS. Stack and
    /// input handles are private so callers cannot inject a forged prefix hash.
    pub fn step(&mut self) -> usize {
        let mut used = 0;
        while let Some(&(path, phase)) = self.pending.last() {
            let node = self.node(path);
            let cost = match &node.kind {
                NodeKind::Leaf(bits) => bits.len() + 1,
                NodeKind::Branch(_, _) => {
                    if phase == 2 {
                        9
                    } else {
                        1
                    }
                }
            };
            if used + cost > Self::STEP_WORDS {
                break;
            }
            used += cost;
            match &node.kind {
                NodeKind::Leaf(_) => {
                    // A leaf has at most PAGE cells. The budget is charged even
                    // when another immutable reader has already cached it.
                    let hash = node.digest();
                    self.pending.pop();
                    self.hashes.push(hash);
                }
                NodeKind::Branch(_, _) if phase == 0 => {
                    self.pending.last_mut().unwrap().1 = 1;
                    self.pending.push((path * 2, 0));
                }
                NodeKind::Branch(_, _) if phase == 1 => {
                    self.pending.last_mut().unwrap().1 = 2;
                    self.pending.push((path * 2 + 1, 0));
                }
                NodeKind::Branch(_, _) => {
                    let right = self.hashes[self.hashes.len() - 1];
                    let left = self.hashes[self.hashes.len() - 2];
                    let mut hash = Sha256::new();
                    hash.update([1]);
                    hash.update(left);
                    hash.update(right);
                    let hash: [u8; 32] = hash.finalize().into();
                    // VM-private snapshots retain computed prefix hashes. The
                    // public untrusted restore route requires independent child
                    // cache confirmation before publishing any supplied prefix.
                    let NodeKind::Branch(a, b) = &node.kind else {
                        unreachable!()
                    };
                    if self.trusted_prefix
                        || (a.hash.get() == Some(&left) && b.hash.get() == Some(&right))
                    {
                        let _ = node.hash.set(hash);
                    }
                    self.hashes.truncate(self.hashes.len() - 2);
                    self.hashes.push(hash);
                    self.pending.pop();
                }
            }
        }
        if self.pending.is_empty() && self.result.is_none() {
            self.result = Some(self.hashes.pop().unwrap());
        }
        used
    }

    /// Bounded VM snapshot payload. Array pages are held separately so ordinary
    /// GC/history accounting sees the complete immutable storage owner.
    pub fn snapshot(&self) -> (&Array, Vec<u8>) {
        let mut bytes = vec![
            1,
            self.pending.len() as u8,
            self.hashes.len() as u8,
            self.result.is_some() as u8,
        ];
        for &(path, phase) in &self.pending {
            bytes.extend_from_slice(&(path as u32).to_le_bytes());
            bytes.push(phase);
        }
        for hash in &self.hashes {
            bytes.extend_from_slice(hash);
        }
        if let Some(hash) = self.result {
            bytes.extend_from_slice(&hash);
        }
        (&self.array, bytes)
    }

    /// Used only for the compiler's reserved private work type. Reject malformed
    /// lengths, paths, phases and stack shapes before traversal can index them.
    pub fn restore(array: &Array, bytes: &[u8]) -> Result<Self> {
        if array.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if bytes.len() < 4 || bytes[0] != 1 || bytes[3] > 1 {
            return Err(Error::Domain);
        }
        let pending_len = bytes[1] as usize;
        let hashes_len = bytes[2] as usize;
        if pending_len > array.buffer.height + 1
            || hashes_len > array.buffer.height + 1
            || bytes.len() != 4 + pending_len * 5 + hashes_len * 32 + bytes[3] as usize * 32
        {
            return Err(Error::Domain);
        }
        let mut pending = Vec::with_capacity(pending_len);
        let mut hashes = Vec::with_capacity(hashes_len);
        let mut cursor = 4;
        for _ in 0..pending_len {
            let path = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            let phase = bytes[cursor + 4];
            pending.push((path, phase));
            cursor += 5;
        }
        for _ in 0..hashes_len {
            hashes.push(bytes[cursor..cursor + 32].try_into().unwrap());
            cursor += 32;
        }
        let result = if bytes[3] == 1 {
            Some(bytes[cursor..cursor + 32].try_into().unwrap())
        } else {
            None
        };
        if result.is_some() {
            if !pending.is_empty() || !hashes.is_empty() {
                return Err(Error::Domain);
            }
        } else {
            if pending.first().map(|f| f.0) != Some(1) {
                return Err(Error::Domain);
            }
            let mut node = array.buffer.root.as_ref();
            let mut expected_hashes = 0;
            for (i, &(path, phase)) in pending.iter().enumerate() {
                match &node.kind {
                    NodeKind::Leaf(_) => {
                        if i + 1 != pending.len() || phase != 0 {
                            return Err(Error::Domain);
                        }
                    }
                    NodeKind::Branch(left, right) => {
                        if phase > 2 {
                            return Err(Error::Domain);
                        }
                        if i + 1 < pending.len() {
                            if phase == 0 || pending[i + 1].0 != path * 2 + usize::from(phase == 2)
                            {
                                return Err(Error::Domain);
                            }
                            if phase == 2 {
                                expected_hashes += 1;
                                node = right;
                            } else {
                                node = left;
                            }
                        } else {
                            expected_hashes += phase as usize;
                        }
                    }
                }
            }
            if hashes_len != expected_hashes {
                return Err(Error::Domain);
            }
        }
        Ok(Self {
            array: array.clone(),
            pending,
            hashes,
            result,
            trusted_prefix: false,
        })
    }

    /// Restore the compiler's reserved, inaccessible work fields without losing
    /// the already-computed prefix when numeric pages have been decoded cold.
    ///
    /// # Safety
    /// `bytes` must be an unmodified snapshot from a trusted chain starting at
    /// `new` and advancing only through `step` or this private restore, bound to
    /// exactly the same backing storage contents. It must not come from user
    /// bytes or an unverified host input. This permits publishing its computed
    /// hashes into the shared immutable storage cache; ordinary embedding callers
    /// must use `restore`, which does not trust supplied prefix hashes.
    pub unsafe fn restore_private_snapshot(array: &Array, bytes: &[u8]) -> Result<Self> {
        let mut work = Self::restore(array, bytes)?;
        work.trusted_prefix = true;
        if let Some(hash) = work.result {
            let _ = work.array.buffer.root.hash.set(hash);
        }
        Ok(work)
    }

    pub fn done(&self) -> bool {
        self.result.is_some()
    }

    pub fn array(&self) -> &Array {
        &self.array
    }

    pub fn key(&self, tag: &str, index: i64, left: &[u8], right: &[u8]) -> Result<[u8; 32]> {
        let hash = self.result.ok_or(Error::Domain)?;
        self.array
            .tensor_key_with_digest(tag, index, left, right, hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn work_stack_is_bounded_even_for_full_sized_shared_storage() {
        let array = Array::zeros(DType::Float64, vec![MAX_ELEMENTS]).unwrap();
        let mut work = TensorIdentityWork::new(&array).unwrap();
        let mut steps = 0;
        while !work.done() {
            assert!(work.step() <= TensorIdentityWork::STEP_WORDS);
            assert!(work.pending.len() <= array.buffer.height + 1);
            assert!(work.hashes.len() <= array.buffer.height + 1);
            steps += 1;
        }
        assert!(steps > 1000); // cache/DAG sharing does not skip VM handoffs
        assert_eq!(
            work.key("max", 0, &[], &[]).unwrap(),
            array.tensor_key("max", 0, &[], &[]).unwrap()
        );
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn snapshots_resume_without_warm_cache_and_reject_invalid_frames() {
        let array =
            Array::floats(vec![4097], &(0..4097).map(|x| x as f64).collect::<Vec<_>>()).unwrap();
        let wire = serde_json::to_vec(&array).unwrap();
        let mut work = TensorIdentityWork::new(&array).unwrap();
        while !work.done() {
            work.step();
            let (_, bytes) = work.snapshot();
            assert!(bytes.len() < 2048);
            let cold: Array = serde_json::from_slice(&wire).unwrap();
            let mut restored = TensorIdentityWork::restore(&cold, &bytes).unwrap();
            while !restored.done() {
                restored.step();
            }
            assert_eq!(
                restored.key("x", 0, &[], &[]).unwrap(),
                array.tensor_key("x", 0, &[], &[]).unwrap()
            );
            let mut invalid = bytes.clone();
            invalid[0] = 2;
            assert!(TensorIdentityWork::restore(&array, &invalid).is_err());
            invalid = bytes.clone();
            invalid.push(0);
            assert!(TensorIdentityWork::restore(&array, &invalid).is_err());
            if bytes[1] > 0 {
                invalid = bytes.clone();
                invalid[4..8].copy_from_slice(&0u32.to_le_bytes());
                assert!(TensorIdentityWork::restore(&array, &invalid).is_err());
                invalid = bytes.clone();
                invalid[8] = 3;
                assert!(TensorIdentityWork::restore(&array, &invalid).is_err());
            }
        }
    }
}

#[cfg(test)]
mod finite_tests {
    use super::*;
    #[test]
    fn validation_stops_at_late_nonfinite_in_view_order_without_prefix_scans() {
        let mut values = vec![1.0; 8193];
        values[8192] = f64::NAN;
        let array = Array::floats(vec![8193], &values).unwrap();
        assert_eq!(array.check_finite_step(0), Ok(4096));
        assert_eq!(array.check_finite_step(4096), Ok(8192));
        assert_eq!(array.check_finite_step(8192), Err(Error::NonFinite));
        assert_eq!(array.check_finite_step(9000), Err(Error::Index));
        let reversed = array.slice(0, 8192, 8193, -1).unwrap();
        assert_eq!(reversed.check_finite_step(0), Err(Error::NonFinite));
        let empty = Array::zeros(DType::Float64, vec![0]).unwrap();
        assert_eq!(empty.check_finite_step(0), Ok(0));
        assert_eq!(
            Array::integers(vec![1], &[1]).unwrap().check_finite_step(0),
            Err(Error::Type)
        );
    }
}
#[cfg(test)]
mod snapshot_integrity_tests {
    use super::*;
    #[test]
    fn malformed_prefix_cannot_poison_shared_array_digest_cache() {
        let array =
            Array::floats(vec![8193], &(0..8193).map(|x| x as f64).collect::<Vec<_>>()).unwrap();
        let wire = serde_json::to_vec(&array).unwrap();
        let mut original = TensorIdentityWork::new(&array).unwrap();
        original.step();
        let (_, mut state) = original.snapshot();
        assert!(state[2] > 0);
        let first_hash = 4 + state[1] as usize * 5;
        state[first_hash] ^= 1;
        let cold: Array = serde_json::from_slice(&wire).unwrap();
        let mut restored = TensorIdentityWork::restore(&cold, &state).unwrap();
        while !restored.done() {
            restored.step();
        }
        assert_eq!(
            cold.buffer.root.digest(),
            super::super::digest_tests::legacy_digest(&cold.buffer.root)
        );
    }
}

#[cfg(test)]
mod private_restore_tests {
    use super::*;
    #[test]
    fn private_cold_restoration_finishes_with_cached_root_without_extra_storage_scan() {
        let array =
            Array::floats(vec![8193], &(0..8193).map(|n| n as f64).collect::<Vec<_>>()).unwrap();
        let wire = serde_json::to_vec(&array).unwrap();
        let mut work = TensorIdentityWork::new(&array).unwrap();
        loop {
            let (_, bytes) = work.snapshot();
            let cold: Array = serde_json::from_slice(&wire).unwrap();
            assert!(cold.buffer.root.hash.get().is_none());
            // SAFETY: an unchanged native snapshot and byte-identical storage.
            let mut resumed =
                unsafe { TensorIdentityWork::restore_private_snapshot(&cold, &bytes) }.unwrap();
            while !resumed.done() {
                assert!(resumed.step() <= TensorIdentityWork::STEP_WORDS);
            }
            let key = resumed.key("x", 0, &[], &[]).unwrap();
            assert!(cold.buffer.root.hash.get().is_some());
            assert_eq!(key, cold.tensor_key("x", 0, &[], &[]).unwrap());
            assert_eq!(
                cold.buffer.root.hash.get().unwrap(),
                &super::super::digest_tests::legacy_digest(&cold.buffer.root)
            );
            if work.done() {
                break;
            }
            work.step();
        }
    }
}
