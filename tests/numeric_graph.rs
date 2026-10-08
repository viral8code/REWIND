use rewind::numeric::{graph_adjacency, graph_bfs, Array, DType, Error, MAX_GRAPH_ITEMS};
use std::collections::VecDeque;
fn ints(v: &[i64]) -> Array {
    Array::integers(vec![v.len()], v).unwrap()
}
#[test]
fn random_directed_graphs_match_independent_queue_distances_and_input_order() {
    for seed in 1..=24_u64 {
        let vertices = (seed as usize * 7) % 61 + 1;
        let mut state = seed;
        let mut froms = Vec::new();
        let mut tos = Vec::new();
        let mut expected = vec![Vec::new(); vertices];
        for _ in 0..vertices * 4 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let from = (state >> 32) as usize % vertices;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let to = (state >> 32) as usize % vertices;
            froms.push(from as i64);
            tos.push(to as i64);
            expected[from].push(to);
        }
        let (heads, links) = graph_adjacency(vertices, &ints(&froms), &ints(&tos)).unwrap();
        for from in 0..vertices {
            let mut index = heads.integer_flat(from).unwrap();
            let mut actual = Vec::new();
            while index != 0 {
                actual.push(tos[index as usize - 1] as usize);
                index = links.integer_flat(index as usize - 1).unwrap();
            }
            let want: Vec<_> = expected[from].iter().rev().copied().collect();
            assert_eq!(actual, want);
        }
        for source in [0, vertices / 2, vertices - 1] {
            let mut distance = vec![-1; vertices];
            distance[source] = 0;
            let mut q = VecDeque::from([source]);
            while let Some(from) = q.pop_front() {
                for &to in &expected[from] {
                    if distance[to] == -1 {
                        distance[to] = distance[from] + 1;
                        q.push_back(to);
                    }
                }
            }
            let actual = graph_bfs(&heads, &ints(&tos), &links, tos.len(), source)
                .unwrap()
                .integer_values()
                .unwrap();
            assert_eq!(actual, distance, "seed{seed}/source{source}");
        }
    }
}
#[test]
fn strided_views_empty_graphs_and_cow_storage_keep_their_contract() {
    let froms = ints(&[9, 0, 9, 1, 9, 0, 9, 3])
        .slice(0, 1, 4, 2)
        .unwrap()
        .slice(0, 3, 4, -1)
        .unwrap();
    let tos = ints(&[9, 1, 9, 2, 9, 3, 9, 3])
        .slice(0, 1, 4, 2)
        .unwrap()
        .slice(0, 3, 4, -1)
        .unwrap();
    let (heads, links) = graph_adjacency(4, &froms, &tos).unwrap();
    assert_eq!(heads.integer_values().unwrap(), vec![4, 3, 0, 1]);
    assert_eq!(links.integer_values().unwrap(), vec![0, 0, 0, 2]);
    assert_eq!(
        graph_bfs(&heads, &tos, &links, 4, 0)
            .unwrap()
            .integer_values()
            .unwrap(),
        vec![0, 1, 2, 1]
    );
    let mut changed = heads.clone();
    changed.set_integer_flat(0, 0).unwrap();
    assert_eq!(heads.integer_flat(0), Ok(4));
    let restored: Array = serde_json::from_slice(&serde_json::to_vec(&heads).unwrap()).unwrap();
    assert_eq!(restored, heads);
    let (h, l) = graph_adjacency(0, &ints(&[]), &ints(&[])).unwrap();
    assert_eq!(h.len(), 0);
    assert_eq!(l.len(), 0);
    assert_eq!(graph_bfs(&h, &ints(&[]), &l, 0, 0), Err(Error::Index));
    let (h, l) = graph_adjacency(3, &ints(&[]), &ints(&[])).unwrap();
    assert_eq!(
        graph_bfs(&h, &ints(&[]), &l, 0, 1)
            .unwrap()
            .integer_values()
            .unwrap(),
        vec![-1, 0, -1]
    );
}
#[test]
fn malformed_inputs_are_rejected_without_unbounded_link_walks() {
    assert_eq!(
        graph_adjacency(MAX_GRAPH_ITEMS + 1, &ints(&[]), &ints(&[])),
        Err(Error::Size)
    );
    assert_eq!(
        graph_adjacency(1, &ints(&[0]), &ints(&[])),
        Err(Error::Shape)
    );
    assert_eq!(
        graph_adjacency(1, &ints(&[-1]), &ints(&[0])),
        Err(Error::Index)
    );
    assert_eq!(
        graph_adjacency(1, &ints(&[0]), &ints(&[i64::MAX])),
        Err(Error::Index)
    );
    assert_eq!(
        graph_adjacency(
            1,
            &Array::zeros(DType::Float64, vec![1]).unwrap(),
            &ints(&[0])
        ),
        Err(Error::Type)
    );
    assert_eq!(
        graph_adjacency(
            1,
            &Array::zeros(DType::Int64, vec![1, 1]).unwrap(),
            &ints(&[0])
        ),
        Err(Error::Shape)
    );
    for (heads, tos, links, edges, error) in [
        (vec![-1], vec![0], vec![0], 1, Error::Index),
        (vec![2], vec![0], vec![0], 1, Error::Index),
        (vec![1], vec![0], vec![1], 1, Error::Domain),
        (vec![1, 1], vec![0], vec![0], 1, Error::Domain),
        (vec![0], vec![0], vec![0], 1, Error::Domain),
        (vec![1], vec![-1], vec![0], 1, Error::Index),
        (vec![1], vec![0], vec![-1], 1, Error::Domain),
        (vec![0], vec![0], vec![0], 2, Error::Index),
    ] {
        assert_eq!(
            graph_bfs(&ints(&heads), &ints(&tos), &ints(&links), edges, 0),
            Err(error)
        );
    }
    assert_eq!(
        graph_bfs(&ints(&[0]), &ints(&[]), &ints(&[0]), 0, 0),
        Err(Error::Shape)
    );
    // Untouched capacity is not an edge; validate/traverse only the admitted prefix.
    assert_eq!(
        graph_bfs(&ints(&[1]), &ints(&[0, -1]), &ints(&[0, -1]), 1, 0)
            .unwrap()
            .integer_values()
            .unwrap(),
        vec![0]
    );
}
#[test]
fn two_hundred_thousand_vertex_chain_runs_in_native_storage() {
    let n = 200000;
    let froms: Vec<_> = (0..n - 1).map(|v| v as i64).collect();
    let tos: Vec<_> = (1..n).map(|v| v as i64).collect();
    let (h, l) = graph_adjacency(n, &ints(&froms), &ints(&tos)).unwrap();
    let distance = graph_bfs(&h, &ints(&tos), &l, n - 1, 0).unwrap();
    assert_eq!(distance.integer_flat(0), Ok(0));
    assert_eq!(distance.integer_flat(n - 1), Ok(n as i64 - 1));
    assert!(h.storage_bytes() + l.storage_bytes() + distance.storage_bytes() < 16 * 1024 * 1024);
}

#[test]
fn native_integer_progressions_check_extremes_before_allocation() {
    assert_eq!(
        Array::integer_range(9, -3, 5)
            .unwrap()
            .integer_values()
            .unwrap(),
        vec![9, 6, 3, 0, -3]
    );
    assert_eq!(
        Array::integer_range(i64::MAX, 0, 3)
            .unwrap()
            .integer_values()
            .unwrap(),
        vec![i64::MAX; 3]
    );
    assert_eq!(Array::integer_range(i64::MIN, -1, 0).unwrap().len(), 0);
    assert_eq!(
        Array::integer_range(i64::MIN, 1, 2)
            .unwrap()
            .integer_values()
            .unwrap(),
        vec![i64::MIN, i64::MIN + 1]
    );
    assert_eq!(Array::integer_range(i64::MAX, 1, 2), Err(Error::Overflow));
    assert_eq!(Array::integer_range(i64::MIN, -1, 2), Err(Error::Overflow));
    assert_eq!(
        Array::integer_range(0, 1, rewind::numeric::MAX_ELEMENTS + 1),
        Err(Error::Size)
    );
}

#[test]
fn zero_progression_uses_virtual_storage_and_point_updates_preserve_it() {
    let original = Array::integer_range(0, 0, 1_000_000).unwrap();
    assert_eq!(
        original.storage_bytes(),
        Array::zeros(DType::Int64, vec![1_000_000])
            .unwrap()
            .storage_bytes()
    );
    let root = std::env::temp_dir().join(format!("rewind-zero-range-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut runtime = rewind::Runtime::new(&root).unwrap();
    runtime.enable_allocation_accounting();
    runtime.enable_numeric_accounting();
    runtime
        .alloc(rewind::Value::NumericArray(original.clone()))
        .unwrap();
    assert!(runtime.retained_numeric_bytes() < 65536);
    let mut changed = original.clone();
    changed.set_integer_flat(999999, 42).unwrap();
    assert_eq!(changed.integer_flat(999999), Ok(42));
    assert_eq!(original.integer_flat(999999), Ok(0));
    drop(runtime);
    std::fs::remove_dir_all(root).unwrap();
}
