use super::*;
fn finish_sum(array: &Array) -> f64 {
    let mut state = KernelProgress::initial();
    loop {
        let previous = state.cursor;
        state = array.sum_step(state).unwrap();
        assert!(state.cursor - previous <= COOPERATIVE_MACS);
        if state.cursor == array.len() {
            return state.sum + state.correction;
        }
    }
}
fn finish_moments(array: &Array, ddof: usize) -> (f64, f64) {
    let mut state = MomentsProgress::initial();
    loop {
        let previous = state.cursor;
        state = array.moments_step(ddof, state).unwrap();
        assert!(state.cursor - previous <= COOPERATIVE_MACS);
        if state.cursor == array.len() {
            return (state.mean, state.variance(array.len(), ddof).unwrap());
        }
    }
}
#[test]
fn sum_chunks_keep_compensation_order_across_boundaries_and_views() {
    let values: Vec<f64> = (0..8193).map(|i| [1e16, 1.0, -1e16][i % 3]).collect();
    let input = Array::floats(vec![8193], &values).unwrap();
    let reversed = input.slice(0, 8192, 8193, -1).unwrap();
    for view in [&input, &reversed] {
        assert_eq!(finish_sum(view), 2731.0);
        assert_eq!(finish_sum(view).to_bits(), view.sum().unwrap().to_bits());
    }
    let matrix = input
        .reshape(vec![3, 2731])
        .unwrap()
        .transpose(&[1, 0])
        .unwrap();
    assert_eq!(
        finish_sum(&matrix).to_bits(),
        matrix.sum().unwrap().to_bits()
    );
    let scalar = Array::floats(vec![], &[-0.0]).unwrap();
    assert_eq!(
        finish_sum(&scalar).to_bits(),
        scalar.sum().unwrap().to_bits()
    );
    let empty = Array::zeros(DType::Float64, vec![0]).unwrap();
    assert_eq!(finish_sum(&empty), 0.0);
    let mut changed = input.clone();
    changed.set_float(&[0], 42.0).unwrap();
    assert_eq!(input.float_flat(0).unwrap(), 1e16);
}
#[test]
fn moments_chunks_match_independent_integer_moments_and_synchronous_bits() {
    let values: Vec<f64> = (0..16448).map(|i| (i % 65) as f64 - 32.0).collect();
    let source = Array::floats(vec![64, 257], &values).unwrap();
    let transposed = source.transpose(&[1, 0]).unwrap();
    for view in [&source, &transposed] {
        let expected_mean = values.iter().sum::<f64>() / values.len() as f64;
        let squares = values
            .iter()
            .map(|v| (v - expected_mean) * (v - expected_mean))
            .sum::<f64>();
        for ddof in [0, 1, view.len() - 1] {
            let actual = finish_moments(view, ddof);
            let expected = squares / (view.len() - ddof) as f64;
            assert!((actual.0 - expected_mean).abs() < 1e-12);
            assert!((actual.1 - expected).abs() < expected.abs().max(1.0) * 1e-12);
            let sync = view.mean_variance(ddof).unwrap();
            assert_eq!(actual.0.to_bits(), sync.0.to_bits());
            assert_eq!(actual.1.to_bits(), sync.1.to_bits());
        }
    }
    let value = Array::floats(vec![1], &[0.5])
        .unwrap()
        .broadcast(vec![8193])
        .unwrap();
    assert_eq!(finish_moments(&value, 0), (0.5, 0.0));
    let scalar = Array::floats(vec![], &[7.0]).unwrap();
    assert_eq!(finish_moments(&scalar, 0), (7.0, 0.0));
}
#[test]
fn reduction_errors_and_invalid_states_never_modify_inputs() {
    let input = Array::zeros(DType::Float64, vec![8193]).unwrap();
    assert!(matches!(
        input.sum_step(KernelProgress {
            cursor: 8194,
            ..KernelProgress::initial()
        }),
        Err(Error::Index)
    ));
    assert!(matches!(
        input.sum_step(KernelProgress {
            sum: f64::INFINITY,
            ..KernelProgress::initial()
        }),
        Err(Error::NonFinite)
    ));
    assert!(matches!(
        input.moments_step(8193, MomentsProgress::initial()),
        Err(Error::Empty)
    ));
    assert!(matches!(
        input.moments_step(
            0,
            MomentsProgress {
                cursor: 8194,
                ..MomentsProgress::initial()
            }
        ),
        Err(Error::Index)
    ));
    assert!(matches!(
        input.moments_step(
            0,
            MomentsProgress {
                m2: f64::NAN,
                ..MomentsProgress::initial()
            }
        ),
        Err(Error::NonFinite)
    ));
    let empty = Array::zeros(DType::Float64, vec![0]).unwrap();
    assert!(matches!(
        empty.moments_step(0, MomentsProgress::initial()),
        Err(Error::Empty)
    ));
    let integer = Array::zeros(DType::Int64, vec![1]).unwrap();
    assert!(matches!(
        integer.sum_step(KernelProgress::initial()),
        Err(Error::Type)
    ));
    assert!(matches!(
        integer.moments_step(0, MomentsProgress::initial()),
        Err(Error::Type)
    ));
    let mut values = vec![0.0; 8193];
    values[8191] = f64::MAX;
    values[8192] = f64::MAX;
    let huge = Array::floats(vec![8193], &values).unwrap();
    let state = huge.sum_step(KernelProgress::initial()).unwrap();
    let state = huge.sum_step(state).unwrap();
    assert!(matches!(huge.sum_step(state), Err(Error::Overflow)));
    assert!(matches!(huge.sum(), Err(Error::Overflow)));
    let opposite = Array::floats(vec![2], &[f64::MAX, -f64::MAX]).unwrap();
    assert!(matches!(
        opposite.moments_step(0, MomentsProgress::initial()),
        Err(Error::Overflow)
    ));
    assert!(matches!(opposite.mean_variance(0), Err(Error::Overflow)));
    assert_eq!(input.float_flat(8192).unwrap(), 0.0);
    let bytes = serde_json::to_vec(&huge).unwrap();
    let restored: Array = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.float_flat(8192).unwrap(), f64::MAX);
}
