use super::*;

// Independent eager implementation of the published storage digest format.
// It does not consult or populate any production digest cache.
pub(super) fn legacy_digest(node: &Node) -> [u8; 32] {
    let mut h = Sha256::new();
    match &node.kind {
        NodeKind::Leaf(bits) => {
            h.update([0]);
            h.update((bits.len() as u64).to_le_bytes());
            for bit in bits {
                h.update(bit.to_le_bytes());
            }
        }
        NodeKind::Branch(a, b) => {
            h.update([1]);
            h.update(legacy_digest(a));
            h.update(legacy_digest(b));
        }
    }
    h.finalize().into()
}

#[test]
fn cached_and_uncached_cow_versions_keep_legacy_identity() {
    let mut a = Array::integers(vec![769], &(0..769).collect::<Vec<i64>>()).unwrap();
    assert!(a.buffer.root.hash.get().is_none());
    let uncached = a.clone();
    let original = legacy_digest(&a.buffer.root);
    assert_eq!(a.buffer.root.digest(), original);
    let cached = a.clone();
    a.set_integer(&[256], -99).unwrap();
    assert!(a.buffer.root.hash.get().is_none());
    assert_eq!(a.buffer.root.digest(), legacy_digest(&a.buffer.root));
    assert_ne!(a.buffer.root.digest(), original);
    let first = a.clone();
    let height = a.buffer.height;
    Node::write_range(&mut a.buffer.root, height, 250, &[10; 32]);
    assert!(a.buffer.root.hash.get().is_none());
    assert_eq!(a.buffer.root.digest(), legacy_digest(&a.buffer.root));
    assert_eq!(cached.buffer.root.digest(), original);
    assert_eq!(uncached.buffer.root.digest(), original);
    assert_eq!(first.integer(&[256]).unwrap(), -99);
    assert_eq!(
        first.buffer.root.digest(),
        legacy_digest(&first.buffer.root)
    );
    assert_eq!(cached, uncached);
    assert_ne!(first, a);
}

#[test]
fn zero_dag_views_and_wire_keep_identity_and_deterministic_work_bound() {
    for len in [0, 1, 255, 256, 257, 512, 769, 4097] {
        let zero = Array::zeros(DType::Int64, vec![len]).unwrap();
        let material = Array::integers(vec![len], &vec![0; len]).unwrap();
        assert_eq!(zero.buffer.root.digest(), legacy_digest(&zero.buffer.root));
        assert_eq!(zero.buffer.root.digest(), material.buffer.root.digest());
        assert!(zero.storage_bytes() <= Array::storage_estimate(len) + 1024);
    }
    // Odd levels add empty right leaves. Cover both balanced and long-tail
    // shapes and use the target's actual native node layout in admission.
    for pages in [1, 2, 3, 5, 9, 17, 33, 65, 129, 257, 513] {
        let len = (pages - 1) * PAGE + 1;
        let material = Array::integers(vec![len], &vec![0; len]).unwrap();
        assert!(
            material.storage_bytes() <= Array::storage_estimate(len),
            "{pages} pages: {} > {}",
            material.storage_bytes(),
            Array::storage_estimate(len)
        );
    }
    assert_eq!(Array::storage_estimate(usize::MAX), usize::MAX);
    let a = Array::zeros(DType::Float64, vec![4096, 4096]).unwrap();
    let view = a.slice(0, 0, 1, 1).unwrap().slice(1, 0, 1, 1).unwrap();
    assert_eq!(view.len(), 1);
    assert_eq!(view.storage_digest_work(), a.storage_digest_work());
    let before = view.storage_digest_work();
    let key = view.tensor_key("param", 0, &[], &[]).unwrap();
    assert_eq!(view.storage_digest_work(), before);
    assert_eq!(view.tensor_key("param", 0, &[], &[]).unwrap(), key);
    let mut small = Array::floats(vec![2, 3], &[-0.0, 1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();
    let saved = small.clone();
    let hash = small.buffer.root.digest();
    small.set_float(&[1, 2], -7.0).unwrap();
    let transposed = small.transpose(&[1, 0]).unwrap();
    let wire = serde_json::to_vec(&transposed).unwrap();
    let decoded: Array = serde_json::from_slice(&wire).unwrap();
    assert!(decoded.buffer.root.hash.get().is_none());
    assert_eq!(transposed, decoded);
    assert_eq!(
        transposed.buffer.root.digest(),
        legacy_digest(&decoded.buffer.root)
    );
    assert_eq!(saved.buffer.root.digest(), hash);
}

#[test]
fn digest_cache_does_not_hold_pages_or_change_runtime_accounting() {
    let ledger = Accounting::default();
    let mut a = Array::integers(vec![4097], &vec![3; 4097]).unwrap();
    ledger.register(&a);
    let initial = ledger.bytes();
    let generation = ledger.generation();
    let hash = a.buffer.root.digest();
    assert_eq!(ledger.bytes(), initial);
    assert_eq!(ledger.generation(), generation);
    let saved = a.clone();
    a.set_integer(&[4096], 7).unwrap();
    ledger.register(&a);
    assert!(ledger.bytes() > initial);
    assert!(ledger.bytes() - initial < 16384);
    assert_ne!(a.buffer.root.digest(), hash);
    assert_eq!(saved.buffer.root.digest(), hash);
    drop(a);
    assert_eq!(ledger.bytes(), initial);
    drop(saved);
    assert_eq!(ledger.bytes(), 0);
}

#[test]
fn simultaneous_immutable_readers_publish_one_matching_digest() {
    let a = Array::integers(vec![4097], &(0..4097).collect::<Vec<i64>>()).unwrap();
    let expected = legacy_digest(&a.buffer.root);
    assert!(a.buffer.root.hash.get().is_none());
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let root = a.buffer.root.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                root.digest()
            })
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), expected);
    }
    assert_eq!(a.buffer.root.hash.get(), Some(&expected));
}

#[test]
fn cooperative_identity_is_bounded_and_independent_of_cache_warmth() {
    for len in [0, 1, 255, 256, 257, 769, 4097, 65537] {
        for materialized in [false, true] {
            let array = if materialized {
                Array::floats(
                    vec![len],
                    &(0..len).map(|i| i as f64 * 0.125 - 3.0).collect::<Vec<_>>(),
                )
                .unwrap()
            } else {
                Array::zeros(DType::Float64, vec![len]).unwrap()
            };
            let expected = legacy_digest(&array.buffer.root);
            let mut cold = TensorIdentityWork::new(&array).unwrap();
            assert_eq!(cold.key("parameter:x", 0, &[], &[]), Err(Error::Domain));
            let mut costs = Vec::new();
            while !cold.done() {
                let used = cold.step();
                assert!(used > 0 && used <= TensorIdentityWork::STEP_WORDS);
                costs.push(used);
            }
            assert_eq!(array.buffer.root.hash.get(), Some(&expected));
            assert_eq!(
                cold.key("parameter:x", 0, &[], &[]).unwrap(),
                array.tensor_key("parameter:x", 0, &[], &[]).unwrap()
            );
            let mut warm = TensorIdentityWork::new(&array).unwrap();
            let mut warm_costs = Vec::new();
            while !warm.done() {
                warm_costs.push(warm.step());
            }
            assert_eq!(costs, warm_costs);
            assert_eq!(cold.step(), 0);
            assert_eq!(cold.key("x", -1, &[], &[]), Err(Error::Domain));
        }
    }
}

#[test]
fn cooperative_identity_restores_shared_views_and_releases_last_reference() {
    let ledger = Accounting::default();
    let mut original = Array::floats(
        vec![257, 257],
        &(0..257 * 257).map(|i| (i % 17) as f64).collect::<Vec<_>>(),
    )
    .unwrap();
    let view = original
        .transpose(&[1, 0])
        .unwrap()
        .slice(0, 1, 2, 1)
        .unwrap();
    ledger.register(&view);
    let retained = ledger.bytes();
    let expected = view.tensor_key("view", 3, &[7; 32], &[9; 32]).unwrap();
    let mut work = TensorIdentityWork::new(&view).unwrap();
    work.step();
    assert!(!work.done());
    let mut restored = work.clone();
    original.set_float(&[0, 0], -5.0).unwrap();
    drop(original);
    drop(view);
    while !work.done() {
        work.step();
    }
    while !restored.done() {
        restored.step();
    }
    assert_eq!(work.key("view", 3, &[7; 32], &[9; 32]).unwrap(), expected);
    assert_eq!(
        restored.key("view", 3, &[7; 32], &[9; 32]).unwrap(),
        expected
    );
    assert_eq!(ledger.bytes(), retained);
    drop(work);
    assert_eq!(ledger.bytes(), retained);
    drop(restored);
    assert_eq!(ledger.bytes(), 0);
    assert!(matches!(
        TensorIdentityWork::new(&Array::integers(vec![1], &[1]).unwrap()),
        Err(Error::Type)
    ));
}
