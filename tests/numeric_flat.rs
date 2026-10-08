use rewind::numeric::{Array, DType, Error};

#[test]
fn logical_flat_order_matches_independent_transpose_and_negative_slice() {
    let values: Vec<i64> = (0..1026).map(|i| i * 19 - 73).collect();
    let base = Array::integers(vec![3, 342], &values).unwrap();
    let transposed = base.transpose(&[1, 0]).unwrap();
    for row in 0..342 {
        for column in 0..3 {
            assert_eq!(
                transposed.integer_flat(row * 3 + column).unwrap(),
                values[column * 342 + row]
            );
        }
    }
    let reverse = transposed.slice(0, 341, 342, -1).unwrap();
    for row in 0..342 {
        for column in 0..3 {
            assert_eq!(
                reverse.integer_flat(row * 3 + column).unwrap(),
                values[column * 342 + 341 - row]
            );
        }
    }
    let mut changed = reverse.clone();
    changed.set_integer_flat(769, i64::MIN).unwrap();
    assert_eq!(changed.integer_flat(769), Ok(i64::MIN));
    assert_eq!(reverse.integer_flat(769).unwrap(), values[342 + 341 - 256]);
    assert_eq!(base.integer(&[1, 85]).unwrap(), values[427]);
    let wire = serde_json::to_vec(&changed).unwrap();
    let restored: Array = serde_json::from_slice(&wire).unwrap();
    assert_eq!(restored, changed);
}

#[test]
fn empty_scalar_broadcast_and_failed_updates_preserve_storage() {
    let empty = Array::zeros(DType::Int64, vec![2, 0, 3]).unwrap();
    assert_eq!(empty.integer_flat(0), Err(Error::Index));
    let mut scalar = Array::integers(vec![], &[i64::MAX]).unwrap();
    assert_eq!(scalar.integer_flat(0), Ok(i64::MAX));
    assert_eq!(scalar.integer_flat(1), Err(Error::Index));
    let before = scalar.clone();
    assert_eq!(scalar.set_integer_flat(usize::MAX, 0), Err(Error::Index));
    assert_eq!(scalar, before);
    let mut broadcast = scalar.broadcast(vec![2, 3]).unwrap();
    for i in 0..6 {
        assert_eq!(broadcast.integer_flat(i), Ok(i64::MAX));
    }
    let before = broadcast.clone();
    assert_eq!(broadcast.set_integer_flat(2, 0), Err(Error::ReadOnly));
    assert_eq!(broadcast, before);
    assert_eq!(scalar.float_flat(0), Err(Error::Type));
    assert_eq!(scalar.set_float_flat(0, 1.0), Err(Error::Type));
    assert_eq!(scalar.integer_flat(0), Ok(i64::MAX));
}

#[test]
fn flat_float_reads_and_cow_preserve_ieee_bits_across_pages_and_wire() {
    let bits: Vec<u64> = (0..513)
        .map(|i| match i {
            0 => (-0.0_f64).to_bits(),
            256 => 0x7ff8_0000_0000_0042,
            512 => f64::NEG_INFINITY.to_bits(),
            _ => (i as f64 * 0.25).to_bits(),
        })
        .collect();
    let original = Array::from_bits(DType::Float64, vec![513], bits.iter().copied()).unwrap();
    let mut changed = original.clone();
    changed.set_float_flat(255, -0.0).unwrap();
    changed
        .set_float_flat(256, f64::from_bits(0x7ff8_0000_0000_0053))
        .unwrap();
    changed.set_float_flat(512, f64::INFINITY).unwrap();
    for (i, &bit) in bits.iter().enumerate() {
        assert_eq!(original.float_flat(i).unwrap().to_bits(), bit);
    }
    assert_eq!(
        changed.float_flat(255).unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        changed.float_flat(256).unwrap().to_bits(),
        0x7ff8_0000_0000_0053
    );
    assert_eq!(
        changed.float_flat(512).unwrap().to_bits(),
        f64::INFINITY.to_bits()
    );
    let restored: Array = serde_json::from_slice(&serde_json::to_vec(&changed).unwrap()).unwrap();
    for i in 0..513 {
        assert_eq!(
            restored.float_flat(i).unwrap().to_bits(),
            changed.float_flat(i).unwrap().to_bits()
        );
    }
    assert_eq!(changed.integer_flat(0), Err(Error::Type));
    let before = changed.clone();
    assert_eq!(changed.set_integer_flat(0, 1), Err(Error::Type));
    assert_eq!(changed, before);
}
