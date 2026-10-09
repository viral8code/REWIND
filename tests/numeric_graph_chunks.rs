use rewind::numeric::{Array, DType, Error, GraphBfsWork, GRAPH_CHUNK, MAX_GRAPH_ITEMS};
use std::collections::VecDeque;

fn ints(values: &[i64]) -> Array {
    Array::integers(vec![values.len()], values).unwrap()
}
fn reference(vertices: usize, froms: &[i64], tos: &[i64], source: usize) -> Vec<i64> {
    let mut edges = vec![Vec::new(); vertices];
    for (&from, &to) in froms.iter().zip(tos) {
        edges[from as usize].push(to as usize);
    }
    let mut distance = vec![-1; vertices];
    distance[source] = 0;
    let mut queue = VecDeque::from([source]);
    while let Some(from) = queue.pop_front() {
        for &to in &edges[from] {
            if distance[to] == -1 {
                distance[to] = distance[from] + 1;
                queue.push_back(to);
            }
        }
    }
    distance
}
fn finish(mut work: GraphBfsWork) -> GraphBfsWork {
    let limit = (work.froms.len() * 3 + work.heads.len() * 4 + 10).div_ceil(GRAPH_CHUNK) + 2;
    for _ in 0..limit {
        if work.done() {
            return work;
        }
        assert!((1..=GRAPH_CHUNK).contains(&work.units_bound()));
        let next = work.step().unwrap();
        assert!(
            next.phase > work.phase
                || next.cursor > work.cursor
                || next.read > work.read
                || next.edge != work.edge
                || next.done()
        );
        work = next;
    }
    panic!("graph work exceeded conservative iteration bound");
}
#[test]
fn cooperative_bulk_bfs_matches_independent_directed_reference() {
    for seed in 1..=20_u64 {
        let n = seed as usize * 11 + 1;
        let mut random = seed;
        let mut froms = Vec::new();
        let mut tos = Vec::new();
        for _ in 0..n * 7 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            froms.push(((random >> 32) as usize % n) as i64);
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            tos.push(((random >> 32) as usize % n) as i64);
        }
        for source in [0, n / 2, n - 1] {
            let work = GraphBfsWork::new(n, &ints(&froms), &ints(&tos), source).unwrap();
            assert_eq!(
                finish(work).result().unwrap().integer_values().unwrap(),
                reference(n, &froms, &tos, source)
            );
        }
    }
}
#[test]
fn bounded_phases_checkpoint_views_and_unreachable_output() {
    let n = 16385;
    let froms: Vec<i64> = (0..n - 2).map(|i| i as i64).collect();
    let tos: Vec<i64> = (1..n - 1).map(|i| i as i64).collect();
    let reversed_froms = ints(&froms)
        .slice(0, froms.len() - 1, froms.len(), -1)
        .unwrap();
    let reversed_tos = ints(&tos).slice(0, tos.len() - 1, tos.len(), -1).unwrap();
    let initial = GraphBfsWork::new(n, &reversed_froms, &reversed_tos, 0).unwrap();
    let first = initial.step().unwrap();
    assert_eq!(first.phase, 0);
    assert_eq!(first.cursor, GRAPH_CHUNK);
    assert_eq!(initial.cursor, 0);
    assert_eq!(initial.heads.integer_flat(0).unwrap(), 0);
    assert!(matches!(initial.result(), Err(Error::Domain)));
    let snapshot = first.clone();
    let mut traversal = first.clone();
    while traversal.phase < 2 {
        traversal = traversal.step().unwrap();
    }
    assert!(traversal.phase < 4);
    let saved_read = traversal.read;
    let saved_write = traversal.write;
    let saved_last = traversal.distance.integer_flat(n - 2).unwrap();
    let resumed = finish(traversal.clone())
        .result()
        .unwrap()
        .integer_values()
        .unwrap();
    assert_eq!(traversal.read, saved_read);
    assert_eq!(traversal.write, saved_write);
    assert_eq!(traversal.distance.integer_flat(n - 2).unwrap(), saved_last);
    assert_eq!(resumed[n - 2], (n - 2) as i64);
    assert_eq!(resumed[n - 1], -1);
    let actual = finish(first).result().unwrap().integer_values().unwrap();
    let again = finish(snapshot.clone())
        .result()
        .unwrap()
        .integer_values()
        .unwrap();
    assert_eq!(actual, again);
    assert_eq!(snapshot.cursor, GRAPH_CHUNK);
    assert_eq!(actual[n - 1], -1);
    for (i, &d) in actual[..n - 1].iter().enumerate() {
        assert_eq!(d, i as i64);
    }
    assert_eq!(reversed_froms.integer_flat(0).unwrap(), (n - 3) as i64);
    let isolated = finish(GraphBfsWork::new(8193, &ints(&[]), &ints(&[]), 4096).unwrap());
    assert_eq!(isolated.result().unwrap().integer_flat(4096).unwrap(), 0);
    assert_eq!(isolated.result().unwrap().integer_flat(0).unwrap(), -1);
    let done = isolated.step().unwrap();
    assert!(done.done());
}
#[test]
fn input_validation_finishes_before_any_adjacency_write_and_rejects_bad_state() {
    let froms = ints(&vec![0; GRAPH_CHUNK + 1]);
    let mut tos_values = vec![0; GRAPH_CHUNK + 1];
    tos_values[GRAPH_CHUNK] = 2;
    let first = GraphBfsWork::new(2, &froms, &ints(&tos_values), 0)
        .unwrap()
        .step()
        .unwrap();
    assert_eq!(first.phase, 0);
    assert_eq!(first.heads.integer_values().unwrap(), vec![0, 0]);
    assert!(matches!(first.step(), Err(Error::Index)));
    assert_eq!(first.cursor, GRAPH_CHUNK);
    assert!(matches!(
        GraphBfsWork::new(0, &ints(&[]), &ints(&[]), 0),
        Err(Error::Index)
    ));
    assert!(matches!(
        GraphBfsWork::new(MAX_GRAPH_ITEMS + 1, &ints(&[]), &ints(&[]), 0),
        Err(Error::Size)
    ));
    assert!(matches!(
        GraphBfsWork::new(2, &froms, &ints(&[]), 0),
        Err(Error::Shape)
    ));
    let float = Array::zeros(DType::Float64, vec![0]).unwrap();
    assert!(matches!(
        GraphBfsWork::new(2, &float, &float, 0),
        Err(Error::Type)
    ));
    let valid = GraphBfsWork::new(2, &ints(&[0]), &ints(&[1]), 0).unwrap();
    let mut bad = valid.clone();
    bad.phase = 6;
    assert!(matches!(bad.step(), Err(Error::Domain)));
    let mut bad = valid.clone();
    bad.cursor = 2;
    assert!(matches!(bad.step(), Err(Error::Domain)));
    let mut bad = valid.clone();
    bad.write = 3;
    assert!(matches!(bad.step(), Err(Error::Domain)));
    let mut bad = valid;
    bad.source = 2;
    assert!(matches!(bad.step(), Err(Error::Index)));
}
