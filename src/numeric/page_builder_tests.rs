use super::*;

#[test]
fn fallible_float_pages_follow_logical_order_and_preserve_cow_and_wire() {
    let values: Vec<f64> = (0..8193).map(|i| (i % 31) as f64 / 16.0 - 1.0).collect();
    let original = Array::floats(vec![values.len()], &values).unwrap();
    let reversed = original
        .slice(0, values.len() - 1, values.len(), -1)
        .unwrap();
    let mut calls = 0;
    let output = reversed
        .map_float(|x| {
            calls += 1;
            Ok(x * x + 2.0 * x)
        })
        .unwrap();
    assert_eq!(calls, values.len());
    let expected: Vec<f64> = values.iter().rev().map(|x| x * x + 2.0 * x).collect();
    let reference = Array::floats(vec![values.len()], &expected).unwrap();
    assert_eq!(
        serde_json::to_vec(&output).unwrap(),
        serde_json::to_vec(&reference).unwrap()
    );
    let one = Array::floats(vec![1], &[0.5]).unwrap();
    let repeated = one.broadcast(vec![values.len()]).unwrap();
    let zipped = reversed.zip_float(&repeated, |a, b| Ok(a + b)).unwrap();
    for i in 0..values.len() {
        assert_eq!(
            zipped.float_flat(i).unwrap(),
            values[values.len() - i - 1] + 0.5
        );
    }
    let mut changed = output.clone();
    changed.set_float(&[0], 42.0).unwrap();
    assert_eq!(output.float_flat(0).unwrap(), expected[0]);
    assert_eq!(original.float_flat(0).unwrap(), values[0]);
}

#[test]
fn float_callbacks_stop_at_first_error_and_validation_precedes_callbacks() {
    let source = Array::zeros(DType::Float64, vec![8193]).unwrap();
    let mut calls = 0;
    let result = source.map_float(|x| {
        calls += 1;
        if calls == 4098 {
            Err(Error::Domain)
        } else {
            Ok(x + 1.0)
        }
    });
    assert!(matches!(result, Err(Error::Domain)));
    assert_eq!(calls, 4098);
    assert_eq!(source.float_flat(4097).unwrap(), 0.0);
    calls = 0;
    let result = source.zip_float(&source, |_, _| {
        calls += 1;
        Ok(f64::NAN)
    });
    assert!(matches!(result, Err(Error::NonFinite)));
    assert_eq!(calls, 1);
    calls = 0;
    let wrong = Array::zeros(DType::Float64, vec![1]).unwrap();
    assert!(matches!(
        source.zip_float(&wrong, |_, _| {
            calls += 1;
            Ok(0.0)
        }),
        Err(Error::Shape)
    ));
    let integer = Array::zeros(DType::Int64, vec![8193]).unwrap();
    assert!(matches!(
        integer.map_float(|_| {
            calls += 1;
            Ok(0.0)
        }),
        Err(Error::Type)
    ));
    assert_eq!(calls, 0);
    let empty = Array::zeros(DType::Float64, vec![0]).unwrap();
    assert_eq!(
        empty
            .map_float(|_| {
                calls += 1;
                Ok(1.0)
            })
            .unwrap()
            .len(),
        0
    );
    assert_eq!(calls, 0);
}

#[test]
fn integer_pages_keep_extreme_bits_and_fail_without_changing_inputs() {
    let values = [i64::MIN, -1, 0, 1, i64::MAX];
    let original = Array::integers(vec![5], &values).unwrap();
    let zero = Array::zeros(DType::Int64, vec![5]).unwrap();
    let result = original.zip_integer(&zero, i64::checked_add).unwrap();
    for (i, value) in values.iter().enumerate() {
        assert_eq!(result.integer_flat(i).unwrap(), *value);
    }
    assert_eq!(
        serde_json::to_vec(&result).unwrap(),
        serde_json::to_vec(&original).unwrap()
    );
    let one = Array::integers(vec![1], &[1])
        .unwrap()
        .broadcast(vec![5])
        .unwrap();
    let mut calls = 0;
    assert!(matches!(
        original.zip_integer(&one, |a, b| {
            calls += 1;
            a.checked_add(b)
        }),
        Err(Error::Overflow)
    ));
    assert_eq!(calls, 5);
    assert_eq!(original.integer_flat(4).unwrap(), i64::MAX);
}

#[test]
fn fallible_builder_checks_declared_length_and_scalar_shape() {
    struct Misreported {
        values: std::vec::IntoIter<Result<u64>>,
        declared: usize,
    }
    impl Iterator for Misreported {
        type Item = Result<u64>;
        fn next(&mut self) -> Option<Self::Item> {
            self.values.next()
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.declared, Some(self.declared))
        }
    }
    impl ExactSizeIterator for Misreported {
        fn len(&self) -> usize {
            self.declared
        }
    }
    for (declared, actual) in [(2, 1), (1, 2), (0, 1)] {
        let values = vec![Ok(0); actual].into_iter();
        assert!(matches!(
            Buffer::from_fallible_bits(Misreported { values, declared }),
            Err(Error::Shape)
        ));
    }
    assert!(matches!(
        Array::from_fallible_bits(DType::Float64, vec![2], [Ok(0)].into_iter()),
        Err(Error::Shape)
    ));
    let scalar = Array::from_fallible_bits(
        DType::Float64,
        vec![],
        [Ok((-0.0f64).to_bits())].into_iter(),
    )
    .unwrap();
    assert_eq!(scalar.float(&[]).unwrap().to_bits(), (-0.0f64).to_bits());
}
