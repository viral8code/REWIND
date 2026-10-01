use super::*;
impl<R: BufRead> Vm<R> {
    fn property_key(&self, value: &Value, at: &Tok) -> Result<String> {
        if program_v09(&self.engine.program) {
            let digest = v09::constant_digest(&self.engine, value)?;
            return digest
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| self.error(at, "invalid property value digest"));
        }
        Ok(packages::hash(
            serde_json::to_vec(value).map_err(|e| self.error(at, e.to_string()))?,
        ))
    }

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
    pub(super) fn property_case(
        &mut self,
        args: &[Value],
        at: &Tok,
        multiple: bool,
    ) -> Result<Value> {
        let [Value::Int(seed), Value::Int(cases), generator, shrinker, predicate] = args else {
            return Err(self.error(at, "invalid property arguments"));
        };
        if !(0..=100_000).contains(cases) {
            return Err(self.error(at, "property cases must be 0..100000"));
        }
        let mut state = *seed as u64;
        for case in 0..*cases {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut bits = state;
            bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d049bb133111eb);
            bits ^= bits >> 31;
            let mut input = self.callback(generator.clone(), vec![Value::Int(bits as i64)], at)?;
            if self.callback(predicate.clone(), vec![input.clone()], at)? == Value::Bool(true) {
                continue;
            }
            let mut seen = BTreeSet::new();
            let mut shrinks = 0;
            let mut evaluations = 0;
            seen.insert(self.property_key(&input, at)?);
            while evaluations < 64 {
                let generated = self.callback(shrinker.clone(), vec![input.clone()], at)?;
                let candidates = if multiple {
                    let value = v05::unfrozen(&generated)
                        .ok_or_else(|| self.error(at, "shrinker must return Frozen<List<T>>"))?;
                    let value = if let Value::HeapRef(id) = value {
                        self.engine
                            .runtime
                            .heap_get(*id)
                            .cloned()
                            .ok_or_else(|| self.error(at, "missing candidate list"))?
                    } else {
                        value.clone()
                    };
                    match value {
                        Value::TypedList(_, items) if items.len() <= 64 => {
                            items.into_iter().collect::<Vec<_>>()
                        }
                        Value::List(items) if items.len() <= 64 => items,
                        _ => return Err(self.error(at, "shrinker candidates exceed budget (64)")),
                    }
                } else {
                    vec![generated]
                };
                let mut next = None;
                for candidate in candidates {
                    let key = self.property_key(&candidate, at)?;
                    if !seen.insert(key) {
                        continue;
                    }
                    if evaluations >= 64 {
                        break;
                    }
                    evaluations += 1;
                    if self.callback(predicate.clone(), vec![candidate.clone()], at)?
                        == Value::Bool(false)
                    {
                        next = Some(candidate);
                        break;
                    }
                }
                let Some(candidate) = next else {
                    break;
                };
                input = candidate;
                shrinks += 1;
            }
            self.audit.push(serde_json::json!({"kind":"propertyFailure","source":at.source,"line":at.line,"column":at.col,"task":self.scheduler.active,"event":self.events.len(),"seed":seed,"case":case,"shrinks":shrinks,"evaluations":evaluations,"strategy":if multiple {"ordered-candidates-v1"}else{"single-v1"},"input":self.engine.runtime.masked_value(&input)}));
            let ty = value_type(&input, &self.engine.runtime);
            return Ok(Value::Result(Err(Box::new(Value::Struct(
                format!("PropertyCase<{ty}>"),
                BTreeMap::from([
                    ("seed".into(), Value::Int(*seed)),
                    ("case".into(), Value::Int(case)),
                    ("input".into(), input),
                    ("shrinks".into(), Value::Int(shrinks)),
                ]),
            )))));
        }
        Ok(Value::Result(Ok(Box::new(Value::Null))))
    }
    pub(super) fn property_int(&mut self, args: &[Value], at: &Tok) -> Result<Value> {
        let [Value::Int(seed), Value::Int(cases), Value::Int(min), Value::Int(max), predicate] =
            args
        else {
            return Err(self.error(at, "invalid propertyInt arguments"));
        };
        if !(0..=100_000).contains(cases) || min > max {
            return Err(self.error(at, "propertyInt requires 0..100000 cases and min <= max"));
        }
        let width = (*max as i128 - *min as i128 + 1) as u128;
        let mut random = *seed as u64;
        for case in 0..*cases {
            // SplitMix64 has a defined state for every seed, including zero.
            random = random.wrapping_add(0x9e3779b97f4a7c15);
            let mut bits = random;
            bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d049bb133111eb);
            bits ^= bits >> 31;
            let mut input = (*min as i128 + (bits as u128 % width) as i128) as i64;
            if self.callback(predicate.clone(), vec![Value::Int(input)], at)? == Value::Bool(true) {
                continue;
            }
            let target = 0i64.clamp(*min, *max);
            let mut shrinks = 0;
            let mut passing = target;
            if input != target
                && self.callback(predicate.clone(), vec![Value::Int(target)], at)?
                    == Value::Bool(false)
            {
                input = target;
                shrinks += 1;
            } else {
                for _ in 0..64 {
                    let distance = input as i128 - passing as i128;
                    if distance.abs() <= 1 {
                        break;
                    }
                    let candidate = (passing as i128 + distance / 2) as i64;
                    if self.callback(predicate.clone(), vec![Value::Int(candidate)], at)?
                        == Value::Bool(false)
                    {
                        input = candidate;
                        shrinks += 1;
                    } else {
                        passing = candidate;
                    }
                }
            }
            return Ok(Value::Result(Err(Box::new(Value::Struct(
                "PropertyFailure".into(),
                BTreeMap::from([
                    ("seed".into(), Value::Int(*seed)),
                    ("case".into(), Value::Int(case)),
                    ("input".into(), Value::Int(input)),
                    ("shrinks".into(), Value::Int(shrinks)),
                ]),
            )))));
        }
        Ok(Value::Result(Ok(Box::new(Value::Null))))
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
        let list = Value::TypedList(item_ty, values.into());
        if method == "collect" {
            return Ok(Some(Value::HeapRef(self.engine.runtime.alloc(list)?)));
        }
        v05::new_iterator(&mut self.engine.runtime, &list)
    }
}
