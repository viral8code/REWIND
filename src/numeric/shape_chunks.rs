//! Bounded logical reshape copies and broadcast-gradient contractions.
use super::*;
pub const SHAPE_CHUNK: usize = 4096;
impl Array {
    pub fn reshape_logical_init(&self, shape: Vec<usize>) -> Result<Self> {
        if count(&shape)? != self.len() {
            return Err(Error::Shape);
        }
        if self.contiguous() {
            self.reshape(shape)
        } else {
            Self::zeros(self.dtype(), shape)
        }
    }
    pub fn reshape_logical_step(
        &self,
        output: &Self,
        cursor: usize,
    ) -> Result<(Self, usize, bool)> {
        if self.dtype() != output.dtype() {
            return Err(Error::Type);
        }
        if self.len() != output.len() {
            return Err(Error::Shape);
        }
        if cursor > self.len() {
            return Err(Error::Index);
        }
        if !output.contiguous() {
            return Err(Error::ReadOnly);
        }
        if self.contiguous() {
            if self.offset != output.offset || !Arc::ptr_eq(&self.buffer.root, &output.buffer.root)
            {
                return Err(Error::Domain);
            }
            return Ok((output.clone(), self.len(), true));
        }
        if output.buffer.len != output.len() {
            return Err(Error::Domain);
        }
        if !output.writable || output.offset != 0 {
            return Err(Error::ReadOnly);
        }
        let end = cursor.saturating_add(SHAPE_CHUNK).min(self.len());
        let mut next = output.clone();
        for flat in cursor..end {
            // Copy the raw bits: logical reshape does not reject NaNs or alter
            // signed zero/payloads, unlike arithmetic vector kernels.
            let bits = self.buffer.get(self.checked_flat_index(flat)?);
            if next.buffer.get(flat) != bits {
                next.buffer.set(flat, bits);
            }
        }
        Ok((next, end, end == self.len()))
    }
}
fn compatible(input: &Array, shape: &[usize]) -> Result<()> {
    if input.dtype() != DType::Float64 {
        return Err(Error::Type);
    }
    count(shape)?;
    if shape.len() > input.shape().len() {
        return Err(Error::Shape);
    }
    let lead = input.shape().len() - shape.len();
    if shape
        .iter()
        .zip(&input.shape()[lead..])
        .any(|(&t, &s)| t != s && t != 1)
    {
        return Err(Error::Shape);
    }
    Ok(())
}
fn target(input: &Array, shape: &[usize], flat: usize) -> usize {
    let lead = input.shape().len() - shape.len();
    let mut remaining = flat;
    let mut index = 0;
    let mut stride = 1;
    for axis in (0..input.shape().len()).rev() {
        let coordinate = remaining % input.shape()[axis];
        remaining /= input.shape()[axis];
        if axis >= lead {
            let dim = shape[axis - lead];
            if dim != 1 {
                index += coordinate * stride;
            }
            stride *= dim;
        }
    }
    index
}
#[derive(Clone)]
pub struct SumShapeProgress {
    pub sums: Array,
    pub corrections: Array,
    pub phase: u8,
    pub cursor: usize,
}
impl SumShapeProgress {
    pub fn new(input: &Array, shape: Vec<usize>) -> Result<Self> {
        compatible(input, &shape)?;
        let sums = Array::zeros(DType::Float64, shape)?;
        Ok(Self {
            corrections: sums.clone(),
            sums,
            phase: 0,
            cursor: 0,
        })
    }
    pub fn validate(&self, input: &Array) -> Result<()> {
        compatible(input, self.sums.shape())?;
        if self.sums.dtype() != DType::Float64 || self.corrections.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        if self.sums.shape() != self.corrections.shape() {
            return Err(Error::Shape);
        }
        if [&self.sums, &self.corrections]
            .iter()
            .any(|a| a.buffer.len != a.len())
        {
            return Err(Error::Domain);
        }
        if [&self.sums, &self.corrections]
            .iter()
            .any(|a| !a.contiguous() || !a.writable || a.offset != 0)
        {
            return Err(Error::ReadOnly);
        }
        if match self.phase {
            0 => self.cursor <= input.len(),
            1 => self.cursor <= self.sums.len(),
            2 => self.cursor == self.sums.len(),
            _ => false,
        } {
            Ok(())
        } else {
            Err(Error::Domain)
        }
    }
    pub fn done(&self) -> bool {
        self.phase == 2
    }
    pub fn units_bound(&self, input: &Array) -> usize {
        match self.phase {
            0 => input
                .len()
                .saturating_sub(self.cursor)
                .saturating_add(self.sums.len())
                .saturating_add(2),
            1 => self
                .sums
                .len()
                .saturating_sub(self.cursor)
                .saturating_add(1),
            _ => 1,
        }
        .min(SHAPE_CHUNK)
        .max(1)
    }
    /// Exact bounded input-page footprint; no full-sized set or output scan.
    /// A fixed 32KiB stack table is covered by the constant temporary allowance.
    pub fn scratch_estimate(&self, input: &Array) -> Result<usize> {
        self.validate(input)?;
        let units = self.units_bound(input);
        let mut pages = [0usize; SHAPE_CHUNK];
        let mut input_pages = 0;
        let mut input_units = 0;
        if self.phase == 0 {
            let end = self.cursor.saturating_add(units).min(input.len());
            input_units = end - self.cursor;
            for (slot, flat) in (self.cursor..end).enumerate() {
                pages[slot] = target(input, self.sums.shape(), flat) / PAGE;
            }
            pages[..input_units].sort_unstable();
            for i in 0..input_units {
                if i == 0 || pages[i] != pages[i - 1] {
                    input_pages += 1;
                }
            }
        }
        let final_units = if self.phase == 1 {
            units.min(self.sums.len().saturating_sub(self.cursor))
        } else if self.phase == 0 && input.len().saturating_sub(self.cursor) < units {
            units.saturating_sub(input_units + 1).min(self.sums.len())
        } else {
            0
        };
        let final_pages = if final_units == 0 {
            0
        } else {
            let start = if self.phase == 1 { self.cursor } else { 0 };
            (start + final_units - 1) / PAGE - start / PAGE + 1
        };
        let copy = |a: &Array, n: usize| {
            Array::storage_estimate(a.len()).min(a.update_estimate().saturating_mul(n))
        };
        Ok(copy(&self.sums, input_pages + final_pages)
            .saturating_add(copy(&self.corrections, input_pages))
            .saturating_add(65536))
    }
    pub fn step(&self, input: &Array) -> Result<Self> {
        self.validate(input)?;
        let mut next = self.clone();
        for _ in 0..SHAPE_CHUNK {
            match next.phase {
                0 => {
                    if next.cursor == input.len() {
                        next.phase = 1;
                        next.cursor = 0;
                        continue;
                    }
                    let x = input.float_flat(next.cursor)?;
                    if !x.is_finite() {
                        return Err(Error::NonFinite);
                    }
                    let index = target(input, next.sums.shape(), next.cursor);
                    let old = next.sums.float_flat(index)?;
                    let correction = next.corrections.float_flat(index)?;
                    let sum = old + x;
                    let c = correction
                        + if old.abs() >= x.abs() {
                            (old - sum) + x
                        } else {
                            (x - sum) + old
                        };
                    if !sum.is_finite() || !c.is_finite() {
                        return Err(Error::Overflow);
                    }
                    if old.to_bits() != sum.to_bits() {
                        next.sums.set_float_flat(index, sum)?;
                    }
                    if correction.to_bits() != c.to_bits() {
                        next.corrections.set_float_flat(index, c)?;
                    }
                    next.cursor += 1;
                }
                1 => {
                    if next.cursor == next.sums.len() {
                        next.phase = 2;
                        continue;
                    }
                    let old = next.sums.float_flat(next.cursor)?;
                    let sum = old + next.corrections.float_flat(next.cursor)?;
                    if !sum.is_finite() {
                        return Err(Error::Overflow);
                    }
                    if old.to_bits() != sum.to_bits() {
                        next.sums.set_float_flat(next.cursor, sum)?;
                    }
                    next.cursor += 1;
                }
                2 => break,
                _ => return Err(Error::Domain),
            }
        }
        next.validate(input)?;
        Ok(next)
    }
}
