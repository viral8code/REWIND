use rewind::numeric::{Array, DType, Error, SumShapeProgress, SHAPE_CHUNK};
trait Wire {
    fn to_wire(&self) -> Vec<u8>;
}
impl Wire for Array {
    fn to_wire(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}
fn finish(input: &Array, mut p: SumShapeProgress) -> SumShapeProgress {
    let bound = (input.len() + p.sums.len() + 3).div_ceil(SHAPE_CHUNK) + 1;
    for _ in 0..bound {
        if p.done() {
            return p;
        }
        assert!((1..=SHAPE_CHUNK).contains(&p.units_bound(input)));
        let next = p.step(input).unwrap();
        assert!(next.phase > p.phase || next.cursor > p.cursor || next.done());
        p = next;
    }
    panic!("contraction exceeded step bound");
}
#[test]
fn cancellation_sensitive_contraction_matches_golden_and_existing_order() {
    let n = 8193;
    let mut values = vec![1e16; n];
    values.extend(vec![1.0; n]);
    values.extend(vec![-1e16; n]);
    let input = Array::floats(vec![3, n], &values).unwrap();
    let initial = SumShapeProgress::new(&input, vec![n]).unwrap();
    let first = initial.step(&input).unwrap();
    assert_eq!(first.phase, 0);
    assert_eq!(first.cursor, SHAPE_CHUNK);
    let checkpoint = first.clone();
    let saved = checkpoint.sums.float_flat(0).unwrap();
    let actual = finish(&input, first).sums;
    assert_eq!(actual.float_values().unwrap(), vec![1.0; n]);
    assert_eq!(checkpoint.sums.float_flat(0).unwrap(), saved);
    assert_eq!(
        actual.to_wire(),
        input.sum_to_shape(vec![n]).unwrap().to_wire()
    );
    assert_eq!(finish(&input, checkpoint).sums.to_wire(), actual.to_wire());
    assert_eq!(initial.sums.float_flat(0).unwrap(), 0.0);
}
#[test]
fn strided_contraction_matches_independent_indexed_sum_and_handles_empty_scalar() {
    let values: Vec<f64> = (1..=24).map(|i| i as f64).collect();
    let input = Array::floats(vec![2, 3, 4], &values)
        .unwrap()
        .transpose(&[2, 0, 1])
        .unwrap();
    let result = finish(
        &input,
        SumShapeProgress::new(&input, vec![4, 1, 3]).unwrap(),
    )
    .sums;
    for k in 0..4 {
        for j in 0..3 {
            assert_eq!(
                result.float(&[k, 0, j]).unwrap(),
                values[j * 4 + k] + values[12 + j * 4 + k]
            );
        }
    }
    assert_eq!(
        result.to_wire(),
        input.sum_to_shape(vec![4, 1, 3]).unwrap().to_wire()
    );
    let empty = Array::zeros(DType::Float64, vec![0, 3]).unwrap();
    assert_eq!(
        finish(&empty, SumShapeProgress::new(&empty, vec![]).unwrap())
            .sums
            .float_flat(0)
            .unwrap(),
        0.0
    );
    assert_eq!(
        finish(&empty, SumShapeProgress::new(&empty, vec![0, 3]).unwrap())
            .sums
            .len(),
        0
    );
    let scalar = Array::floats(vec![], &[7.0]).unwrap();
    assert_eq!(
        finish(&scalar, SumShapeProgress::new(&scalar, vec![]).unwrap())
            .sums
            .float_flat(0)
            .unwrap(),
        7.0
    );
}
#[test]
fn logical_reshape_preserves_raw_bits_views_and_alias_fast_path() {
    let bits = [
        0x7ff8000000001234_u64,
        (-0.0f64).to_bits(),
        1.0f64.to_bits(),
        0xfff0000000000000,
    ];
    let input = Array::floats(vec![2, 2], &bits.map(f64::from_bits))
        .unwrap()
        .transpose(&[1, 0])
        .unwrap();
    let mut output = input.reshape_logical_init(vec![4]).unwrap();
    let mut cursor = 0;
    loop {
        let step = input.reshape_logical_step(&output, cursor).unwrap();
        output = step.0;
        cursor = step.1;
        if step.2 {
            break;
        }
    }
    assert_eq!(
        output.to_wire(),
        input.reshape_logical(vec![4]).unwrap().to_wire()
    );
    let direct = Array::integers(vec![3], &[i64::MIN, 0, i64::MAX]).unwrap();
    let init = direct.reshape_logical_init(vec![1, 3]).unwrap();
    assert!(direct.reshape_logical_step(&init, 0).unwrap().2);
    let values: Vec<i64> = (0..8193).map(|i| i as i64).collect();
    let reversed = Array::integers(vec![8193], &values)
        .unwrap()
        .slice(0, 8192, 8193, -1)
        .unwrap();
    let initial = reversed.reshape_logical_init(vec![3, 2731]).unwrap();
    let (first, cursor, done) = reversed.reshape_logical_step(&initial, 0).unwrap();
    assert_eq!(cursor, SHAPE_CHUNK);
    assert!(!done);
    assert_eq!(first.integer_flat(0).unwrap(), 8192);
    assert_eq!(initial.integer_flat(0).unwrap(), 0);
    let (second, cursor, _) = reversed.reshape_logical_step(&first, cursor).unwrap();
    let (complete, _, done) = reversed.reshape_logical_step(&second, cursor).unwrap();
    assert!(done);
    assert_eq!(
        complete.to_wire(),
        reversed.reshape_logical(vec![3, 2731]).unwrap().to_wire()
    );
}
#[test]
fn bad_shapes_progress_late_nonfinite_and_overflow_do_not_mutate_saved_state() {
    let input = Array::zeros(DType::Float64, vec![2, 3]).unwrap();
    assert!(matches!(
        SumShapeProgress::new(&input, vec![2, 2]),
        Err(Error::Shape)
    ));
    let mut bad = SumShapeProgress::new(&input, vec![1, 3]).unwrap();
    bad.phase = 3;
    assert!(matches!(bad.step(&input), Err(Error::Domain)));
    let mut bad = SumShapeProgress::new(&input, vec![1, 3]).unwrap();
    bad.cursor = 7;
    assert!(matches!(bad.step(&input), Err(Error::Domain)));
    let mut values = vec![1.0; SHAPE_CHUNK + 1];
    values[SHAPE_CHUNK] = f64::NAN;
    let input = Array::floats(vec![values.len()], &values).unwrap();
    let p = SumShapeProgress::new(&input, vec![])
        .unwrap()
        .step(&input)
        .unwrap();
    assert!(matches!(p.step(&input), Err(Error::NonFinite)));
    assert_eq!(p.sums.float_flat(0).unwrap(), SHAPE_CHUNK as f64);
    let input = Array::floats(vec![2], &[f64::MAX, f64::MAX]).unwrap();
    assert!(matches!(
        SumShapeProgress::new(&input, vec![]).unwrap().step(&input),
        Err(Error::Overflow)
    ));
    let input = Array::floats(vec![2], &[1.0, 2.0])
        .unwrap()
        .slice(0, 1, 2, -1)
        .unwrap();
    let backing = Array::zeros(DType::Float64, vec![65536]).unwrap();
    let prefix = backing.slice(0, 0, 2, 1).unwrap();
    assert!(matches!(
        input.reshape_logical_step(&prefix, 0),
        Err(Error::Domain)
    ));
    let mut p = SumShapeProgress::new(&input, vec![2]).unwrap();
    p.sums = prefix;
    assert!(matches!(p.step(&input), Err(Error::Domain)));
}
