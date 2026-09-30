use super::*;
impl<R: BufRead> Vm<R> {
    fn callback(&mut self, f: Value, args: Vec<Value>, at: &Tok) -> Result<Value> {
        let depth = self.frames.len();
        let old = self.callback_stop;
        self.call_value(f, args, at)?;
        self.callback_stop = Some(depth);
        let result = self.run_inner();
        self.callback_stop = old;
        result?;
        if self.frames.len() != depth {
            return Err(self.error(at, "callback did not complete"));
        }
        self.pop()
    }
    pub(super) fn result_adapter(
        &mut self,
        target: &Value,
        method: &str,
        args: &[Value],
        at: &Tok,
    ) -> Result<Option<Value>> {
        let Value::Result(result) = target else {
            return Ok(None);
        };
        let wrap = |r| Some(Value::Result(r));
        Ok(match (method, args, result) {
            ("map", [f], Ok(value)) => wrap(Ok(Box::new(self.callback(
                f.clone(),
                vec![(**value).clone()],
                at,
            )?))),
            ("map", [_], Err(value)) => wrap(Err(value.clone())),
            ("mapErr", [f], Err(value)) => wrap(Err(Box::new(self.callback(
                f.clone(),
                vec![(**value).clone()],
                at,
            )?))),
            ("mapErr", [_], Ok(value)) => wrap(Ok(value.clone())),
            ("andThen", [f], Ok(value)) => {
                Some(self.callback(f.clone(), vec![(**value).clone()], at)?)
            }
            ("andThen", [_], Err(value)) => wrap(Err(value.clone())),
            ("flatten", [], Ok(value)) => Some((**value).clone()),
            ("flatten", [], Err(value)) => wrap(Err(value.clone())),
            _ => None,
        })
    }
    fn next_item(&mut self, target: &Value, at: &Tok) -> Result<Option<Value>> {
        if self.steps == 0 {
            return Err(self.error(at, "ExecutionBudgetExceeded"));
        }
        self.steps -= 1;
        let ty = value_type(target, &self.engine.runtime);
        let methods = matching_methods(&self.engine.program, &ty, "next");
        let value = if let Some(symbol) = methods.first() {
            self.callback(
                Value::Function(symbol.clone(), format!("fn({ty})->Option<Unknown>")),
                vec![target.clone()],
                at,
            )?
        } else {
            v05::iterator_next(&mut self.engine.runtime, target)?
                .ok_or_else(|| self.error(at, "not an Iterator"))?
        };
        match v06::immutable_tuple(value, &self.engine.runtime) {
            Value::Option(v) => Ok(v.map(|v| *v)),
            _ => Err(self.error(at, "Iterator.next must return Option")),
        }
    }
    pub(super) fn iterator_adapter(
        &mut self,
        target: &Value,
        method: &str,
        args: &[Value],
        at: &Tok,
    ) -> Result<Option<Value>> {
        if !matches!(
            method,
            "map" | "filter" | "take" | "fold" | "collect" | "enumerate" | "zip"
        ) {
            return Ok(None);
        }
        let ty = value_type(target, &self.engine.runtime);
        if !ty.starts_with("Iterator<")
            && matching_methods(&self.engine.program, &ty, "next").is_empty()
        {
            return Ok(None);
        }
        let limit = match (method, args) {
            ("take", [Value::Int(n)]) if *n >= 0 => *n as usize,
            ("take", _) => return Err(self.error(at, "take count must be nonnegative")),
            _ => 100_000,
        };
        let mut values = Vec::new();
        let mut acc = args.first().cloned().unwrap_or(Value::Null);
        let mut item_ty = None;
        for index in 0..=limit.min(100_000) {
            if method == "take" && index == limit {
                break;
            }
            let Some(item) = self.next_item(target, at)? else {
                break;
            };
            if index == 100_000 {
                return Err(self.error(at, "IteratorBudgetExceeded: 100000 items"));
            }
            item_ty.get_or_insert_with(|| value_type(&item, &self.engine.runtime));
            match (method, args) {
                ("map", [f]) => values.push(self.callback(f.clone(), vec![item], at)?),
                ("filter", [f]) => {
                    if self.callback(f.clone(), vec![item.clone()], at)? == Value::Bool(true) {
                        values.push(item);
                    }
                }
                ("fold", [_, f]) => acc = self.callback(f.clone(), vec![acc, item], at)?,
                ("enumerate", []) => values.push(Value::Struct(
                    format!("Tuple<Int,{}>", value_type(&item, &self.engine.runtime)),
                    BTreeMap::from([("_0".into(), Value::Int(index as i64)), ("_1".into(), item)]),
                )),
                ("zip", [other]) => {
                    let Some(right) = self.next_item(other, at)? else {
                        break;
                    };
                    values.push(Value::Struct(
                        format!(
                            "Tuple<{},{}>",
                            value_type(&item, &self.engine.runtime),
                            value_type(&right, &self.engine.runtime)
                        ),
                        BTreeMap::from([("_0".into(), item), ("_1".into(), right)]),
                    ));
                }
                ("collect" | "take", _) => values.push(item),
                _ => return Err(self.error(at, "invalid iterator adapter arguments")),
            }
        }
        if method == "fold" {
            return Ok(Some(acc));
        }
        let item_ty = if method == "map" {
            args.first()
                .and_then(|f| function_signature(&value_type(f, &self.engine.runtime)))
                .map(|(_, r)| r)
                .unwrap_or_else(|| "Unknown".into())
        } else {
            v05::iterator_item(&ty)
                .or(item_ty)
                .unwrap_or_else(|| "Unknown".into())
        };
        let item_ty = if method == "enumerate" {
            format!("Tuple<Int,{item_ty}>")
        } else if method == "zip" {
            format!(
                "Tuple<{item_ty},{}>",
                args.first()
                    .and_then(|a| v05::iterator_item(&value_type(a, &self.engine.runtime)))
                    .unwrap_or_else(|| "Unknown".into())
            )
        } else {
            item_ty
        };
        let list = Value::TypedList(item_ty, values);
        if method == "collect" {
            return Ok(Some(Value::HeapRef(self.engine.runtime.alloc(list)?)));
        }
        v05::new_iterator(&mut self.engine.runtime, &list)
    }
}
