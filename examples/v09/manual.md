# Generic record

```rewind
record Box<T:Share>{value:T}
let box=Box((4,true));
assert_eq(box.value._0,4);
```
