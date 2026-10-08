use super::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub enum UnaryOperation {
    Sqrt,
    Exp,
    ExpM1,
    Log,
    Log1p,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Abs,
    Floor,
    Ceil,
    Trunc,
    Round,
    RoundEven,
    Relu,
    ReluGrad,
    Sigmoid,
    SigmoidGrad,
    TanhGrad,
    Reciprocal,
}
impl UnaryOperation {
    pub fn map(name: &str) -> Result<Self> {
        Ok(match name {
            "sqrt" => Self::Sqrt,
            "exp" => Self::Exp,
            "expM1" => Self::ExpM1,
            "log" => Self::Log,
            "log1p" => Self::Log1p,
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "asin" => Self::Asin,
            "acos" => Self::Acos,
            "atan" => Self::Atan,
            "sinh" => Self::Sinh,
            "cosh" => Self::Cosh,
            "tanh" => Self::Tanh,
            "abs" => Self::Abs,
            "floor" => Self::Floor,
            "ceil" => Self::Ceil,
            "trunc" => Self::Trunc,
            "round" => Self::Round,
            "roundEven" => Self::RoundEven,
            _ => return Err(Error::Domain),
        })
    }
    pub fn activation(name: &str) -> Result<Self> {
        Ok(match name {
            "relu" => Self::Relu,
            "reluGrad" => Self::ReluGrad,
            "sigmoid" => Self::Sigmoid,
            "sigmoidGrad" => Self::SigmoidGrad,
            "tanhGrad" => Self::TanhGrad,
            "reciprocal" => Self::Reciprocal,
            _ => return Err(Error::Domain),
        })
    }
    pub fn apply(self, value: f64) -> Result<f64> {
        if !value.is_finite() {
            return Err(Error::NonFinite);
        }
        let output = match self {
            Self::Sqrt => {
                if value < 0.0 {
                    return Err(Error::Domain);
                }
                value.sqrt()
            }
            Self::Exp => value.exp(),
            Self::ExpM1 => value.exp_m1(),
            Self::Log => {
                if value <= 0.0 {
                    return Err(Error::Domain);
                }
                value.ln()
            }
            Self::Log1p => {
                if value <= -1.0 {
                    return Err(Error::Domain);
                }
                value.ln_1p()
            }
            Self::Sin => value.sin(),
            Self::Cos => value.cos(),
            Self::Tan => value.tan(),
            Self::Asin => {
                if value.abs() > 1.0 {
                    return Err(Error::Domain);
                }
                value.asin()
            }
            Self::Acos => {
                if value.abs() > 1.0 {
                    return Err(Error::Domain);
                }
                value.acos()
            }
            Self::Atan => value.atan(),
            Self::Sinh => value.sinh(),
            Self::Cosh => value.cosh(),
            Self::Tanh => value.tanh(),
            Self::Abs => value.abs(),
            Self::Floor => value.floor(),
            Self::Ceil => value.ceil(),
            Self::Trunc => value.trunc(),
            Self::Round => value.round(),
            Self::RoundEven => value.round_ties_even(),
            Self::Relu => value.max(0.0),
            Self::ReluGrad => {
                if value > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Sigmoid => {
                if value >= 0.0 {
                    1.0 / (1.0 + (-value).exp())
                } else {
                    let exp = value.exp();
                    exp / (1.0 + exp)
                }
            }
            Self::SigmoidGrad => {
                if !(0.0..=1.0).contains(&value) {
                    return Err(Error::Domain);
                }
                value * (1.0 - value)
            }
            Self::TanhGrad => {
                if !(-1.0..=1.0).contains(&value) {
                    return Err(Error::Domain);
                }
                1.0 - value * value
            }
            Self::Reciprocal => {
                if value == 0.0 {
                    return Err(Error::Domain);
                }
                1.0 / value
            }
        };
        if output.is_finite() {
            Ok(output)
        } else {
            Err(
                if matches!(
                    self,
                    Self::Relu
                        | Self::ReluGrad
                        | Self::Sigmoid
                        | Self::SigmoidGrad
                        | Self::TanhGrad
                        | Self::Reciprocal
                ) {
                    Error::Overflow
                } else {
                    Error::NonFinite
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numeric::{Array, DType, VectorOperation, COOPERATIVE_MACS};

    #[test]
    fn cooperative_transforms_match_independent_functions_on_reversed_pages() {
        let operations: Vec<(UnaryOperation, fn(f64) -> f64)> = vec![
            (UnaryOperation::Sqrt, f64::sqrt),
            (UnaryOperation::Exp, f64::exp),
            (UnaryOperation::ExpM1, f64::exp_m1),
            (UnaryOperation::Log, f64::ln),
            (UnaryOperation::Log1p, f64::ln_1p),
            (UnaryOperation::Sin, f64::sin),
            (UnaryOperation::Cos, f64::cos),
            (UnaryOperation::Tan, f64::tan),
            (UnaryOperation::Asin, f64::asin),
            (UnaryOperation::Acos, f64::acos),
            (UnaryOperation::Atan, f64::atan),
            (UnaryOperation::Sinh, f64::sinh),
            (UnaryOperation::Cosh, f64::cosh),
            (UnaryOperation::Tanh, f64::tanh),
            (UnaryOperation::Abs, f64::abs),
            (UnaryOperation::Floor, f64::floor),
            (UnaryOperation::Ceil, f64::ceil),
            (UnaryOperation::Trunc, f64::trunc),
            (UnaryOperation::Round, f64::round),
            (UnaryOperation::RoundEven, f64::round_ties_even),
            (UnaryOperation::Relu, |x| x.max(0.0)),
            (
                UnaryOperation::ReluGrad,
                |x| if x > 0.0 { 1.0 } else { 0.0 },
            ),
            (UnaryOperation::Sigmoid, |x| 1.0 / (1.0 + (-x).exp())),
            (UnaryOperation::SigmoidGrad, |x| x * (1.0 - x)),
            (UnaryOperation::TanhGrad, |x| 1.0 - x * x),
            (UnaryOperation::Reciprocal, |x| 1.0 / x),
        ];
        let values: Vec<f64> = (0..8193).map(|i| [0.25, 0.5, 0.75][i % 3]).collect();
        let original = Array::floats(vec![values.len()], &values).unwrap();
        let input = original
            .slice(0, values.len() - 1, values.len(), -1)
            .unwrap();
        for (unary, reference) in operations {
            let operation = VectorOperation::Unary(unary);
            let mut output = input.vector_init(&input, operation).unwrap();
            let (first, first_cursor) = input.vector_step(&input, &output, operation, 0).unwrap();
            assert_eq!(first_cursor, COOPERATIVE_MACS);
            let snapshot = first.clone();
            output = first;
            let mut cursor = first_cursor;
            while cursor < input.len() {
                let (next, end) = input
                    .vector_step(&input, &output, operation, cursor)
                    .unwrap();
                assert!(end > cursor && end - cursor <= COOPERATIVE_MACS);
                output = next;
                cursor = end;
            }
            for i in 0..values.len() {
                let expected = reference(values[values.len() - i - 1]).to_bits();
                assert_eq!(output.float(&[i]).unwrap().to_bits(), expected);
                assert_eq!(
                    snapshot.float(&[i]).unwrap().to_bits(),
                    if i < COOPERATIVE_MACS { expected } else { 0 }
                );
            }
            let wire = serde_json::to_vec(&output).unwrap();
            let restored: Array = serde_json::from_slice(&wire).unwrap();
            assert_eq!(
                restored.float(&[8192]).unwrap().to_bits(),
                output.float(&[8192]).unwrap().to_bits()
            );
        }
        assert_eq!(original.float(&[0]).unwrap(), values[0]);
    }

    #[test]
    fn errors_and_stable_activation_extremes_preserve_previous_contracts() {
        assert_eq!(UnaryOperation::Exp.apply(1000.0), Err(Error::NonFinite));
        assert_eq!(
            UnaryOperation::Reciprocal.apply(1e-320),
            Err(Error::Overflow)
        );
        assert_eq!(UnaryOperation::Sigmoid.apply(1000.0), Ok(1.0));
        assert_eq!(UnaryOperation::Sigmoid.apply(-1000.0), Ok(0.0));
        assert_eq!(UnaryOperation::ReluGrad.apply(-0.0), Ok(0.0));
        assert_eq!(UnaryOperation::TanhGrad.apply(-1.0), Ok(0.0));
        assert_eq!(UnaryOperation::Log.apply(-1.0), Err(Error::Domain));
        assert_eq!(UnaryOperation::Log1p.apply(-1.0), Err(Error::Domain));
        assert_eq!(UnaryOperation::SigmoidGrad.apply(1.01), Err(Error::Domain));
        assert_eq!(UnaryOperation::Sin.apply(f64::NAN), Err(Error::NonFinite));
        assert!(matches!(UnaryOperation::map("relu"), Err(Error::Domain)));
        assert!(matches!(
            UnaryOperation::activation("exp"),
            Err(Error::Domain)
        ));
    }

    #[test]
    fn failing_late_step_cannot_mutate_a_previous_partial_result() {
        let mut values = vec![2.0; 4097];
        values[4096] = -1.0;
        let input = Array::floats(vec![values.len()], &values).unwrap();
        let operation = VectorOperation::Unary(UnaryOperation::Log);
        let output = input.vector_init(&input, operation).unwrap();
        let (partial, cursor) = input.vector_step(&input, &output, operation, 0).unwrap();
        assert!(matches!(
            input.vector_step(&input, &partial, operation, cursor),
            Err(Error::Domain)
        ));
        assert_eq!(partial.float(&[4096]).unwrap(), 0.0);
        assert_eq!(partial.float(&[0]).unwrap(), 2.0f64.ln());
        assert_eq!(output.float(&[0]).unwrap(), 0.0);
        assert!(matches!(
            input.vector_step(
                &input,
                &partial.broadcast(vec![4097]).unwrap(),
                operation,
                0
            ),
            Err(Error::ReadOnly)
        ));
        let integer = Array::zeros(DType::Int64, vec![0]).unwrap();
        assert!(matches!(
            integer.vector_init(&integer, operation),
            Err(Error::Type)
        ));
    }
}
