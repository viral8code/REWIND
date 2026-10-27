//! A small persistent ordered map: cloning is O(1) and a change copies only the nodes on the path
//! from the root to the changed key. The type checker and the ownership analysis branch on their
//! variable tables all the time (every block, closure and match arm); with a plain `BTreeMap` that
//! made a function with many variables quadratic, which a hostile source could use to stall the
//! compiler. The map is an AVL tree whose nodes are shared through `Arc`; iteration is in key
//! order, exactly like `BTreeMap`.
use std::{borrow::Borrow, cmp::Ordering, collections::BTreeMap, fmt, sync::Arc};

struct Node<K, V> {
    key: K,
    val: V,
    left: Link<K, V>,
    right: Link<K, V>,
    height: u8,
    size: usize,
}
impl<K: Clone, V: Clone> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        Node {
            key: self.key.clone(),
            val: self.val.clone(),
            left: self.left.clone(),
            right: self.right.clone(),
            height: self.height,
            size: self.size,
        }
    }
}
type Link<K, V> = Option<Arc<Node<K, V>>>;

fn height<K, V>(link: &Link<K, V>) -> u8 {
    link.as_ref().map_or(0, |n| n.height)
}
fn size<K, V>(link: &Link<K, V>) -> usize {
    link.as_ref().map_or(0, |n| n.size)
}
fn update<K, V>(node: &mut Node<K, V>) {
    node.height = 1 + height(&node.left).max(height(&node.right));
    node.size = 1 + size(&node.left) + size(&node.right);
}
fn balance_factor<K, V>(node: &Node<K, V>) -> i16 {
    i16::from(height(&node.left)) - i16::from(height(&node.right))
}
fn rotate_right<K: Clone, V: Clone>(link: &mut Link<K, V>) {
    let mut top = link.take().expect("rotate_right needs a node");
    let top_node = Arc::make_mut(&mut top);
    let mut pivot = top_node
        .left
        .take()
        .expect("rotate_right needs a left child");
    let pivot_node = Arc::make_mut(&mut pivot);
    top_node.left = pivot_node.right.take();
    update(top_node);
    pivot_node.right = Some(top);
    update(pivot_node);
    *link = Some(pivot);
}
fn rotate_left<K: Clone, V: Clone>(link: &mut Link<K, V>) {
    let mut top = link.take().expect("rotate_left needs a node");
    let top_node = Arc::make_mut(&mut top);
    let mut pivot = top_node
        .right
        .take()
        .expect("rotate_left needs a right child");
    let pivot_node = Arc::make_mut(&mut pivot);
    top_node.right = pivot_node.left.take();
    update(top_node);
    pivot_node.left = Some(top);
    update(pivot_node);
    *link = Some(pivot);
}
/// Restores the AVL invariant at `link` after one of its subtrees changed height by at most one.
fn rebalance<K: Clone, V: Clone>(link: &mut Link<K, V>) {
    let Some(arc) = link.as_mut() else { return };
    let node = Arc::make_mut(arc);
    update(node);
    let factor = balance_factor(node);
    if factor > 1 {
        if let Some(left) = node.left.as_ref() {
            if balance_factor(left) < 0 {
                rotate_left(&mut node.left);
            }
        }
        rotate_right(link);
    } else if factor < -1 {
        if let Some(right) = node.right.as_ref() {
            if balance_factor(right) > 0 {
                rotate_right(&mut node.right);
            }
        }
        rotate_left(link);
    }
}
fn insert<K: Ord + Clone, V: Clone>(link: &mut Link<K, V>, key: K, val: V) -> Option<V> {
    let old = match link {
        None => {
            *link = Some(Arc::new(Node {
                key,
                val,
                left: None,
                right: None,
                height: 1,
                size: 1,
            }));
            return None;
        }
        Some(arc) => {
            let node = Arc::make_mut(arc);
            match key.cmp(&node.key) {
                Ordering::Less => insert(&mut node.left, key, val),
                Ordering::Greater => insert(&mut node.right, key, val),
                Ordering::Equal => return Some(std::mem::replace(&mut node.val, val)),
            }
        }
    };
    rebalance(link);
    old
}
fn pop_min<K: Ord + Clone, V: Clone>(link: &mut Link<K, V>) -> Option<(K, V)> {
    let has_left = link.as_ref()?.left.is_some();
    if !has_left {
        let arc = link.take()?;
        let node = Arc::try_unwrap(arc).unwrap_or_else(|shared| (*shared).clone());
        *link = node.right;
        return Some((node.key, node.val));
    }
    let node = Arc::make_mut(link.as_mut()?);
    let out = pop_min(&mut node.left);
    rebalance(link);
    out
}
fn remove<K, Q, V>(link: &mut Link<K, V>, key: &Q) -> Option<V>
where
    K: Borrow<Q> + Ord + Clone,
    Q: Ord + ?Sized,
    V: Clone,
{
    let ordering = key.cmp(link.as_ref()?.key.borrow());
    let removed = match ordering {
        Ordering::Less => remove(&mut Arc::make_mut(link.as_mut()?).left, key),
        Ordering::Greater => remove(&mut Arc::make_mut(link.as_mut()?).right, key),
        Ordering::Equal => {
            let arc = link.take()?;
            let node = Arc::try_unwrap(arc).unwrap_or_else(|shared| (*shared).clone());
            let value = node.val.clone();
            match (node.left, node.right) {
                (None, right) => *link = right,
                (left, None) => *link = left,
                (left, mut right) => {
                    let (min_key, min_val) = pop_min(&mut right)?;
                    *link = Some(Arc::new(Node {
                        key: min_key,
                        val: min_val,
                        left,
                        right,
                        height: 1,
                        size: 1,
                    }));
                }
            }
            rebalance(link);
            return Some(value);
        }
    };
    rebalance(link);
    removed
}
fn get_mut_in<'a, K, Q, V>(link: &'a mut Link<K, V>, key: &Q) -> Option<&'a mut V>
where
    K: Borrow<Q> + Ord + Clone,
    Q: Ord + ?Sized,
    V: Clone,
{
    let node = Arc::make_mut(link.as_mut()?);
    match key.cmp(node.key.borrow()) {
        Ordering::Less => get_mut_in(&mut node.left, key),
        Ordering::Greater => get_mut_in(&mut node.right, key),
        Ordering::Equal => Some(&mut node.val),
    }
}
/// Builds a balanced tree from items sorted by key without duplicates.
fn build<K, V>(items: &mut std::vec::IntoIter<(K, V)>, count: usize) -> Link<K, V> {
    if count == 0 {
        return None;
    }
    let left_count = (count - 1) / 2;
    let left = build(items, left_count);
    let (key, val) = items.next().expect("count matches the items");
    let right = build(items, count - 1 - left_count);
    let mut node = Node {
        key,
        val,
        left,
        right,
        height: 1,
        size: 1,
    };
    update(&mut node);
    Some(Arc::new(node))
}

/// An ordered map with O(1) clone and O(log n) change (see the module comment).
pub(in crate::v2) struct PMap<K, V> {
    root: Link<K, V>,
}
impl<K, V> Clone for PMap<K, V> {
    fn clone(&self) -> Self {
        PMap {
            root: self.root.clone(),
        }
    }
}
impl<K, V> Default for PMap<K, V> {
    fn default() -> Self {
        PMap { root: None }
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for PMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<K, V> PMap<K, V> {
    #[cfg(test)]
    pub(in crate::v2) fn len(&self) -> usize {
        size(&self.root)
    }
    #[cfg(test)]
    pub(in crate::v2) fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    pub(in crate::v2) fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        let mut link = &self.root;
        while let Some(node) = link {
            match key.cmp(node.key.borrow()) {
                Ordering::Less => link = &node.left,
                Ordering::Greater => link = &node.right,
                Ordering::Equal => return Some(&node.val),
            }
        }
        None
    }
    pub(in crate::v2) fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.get(key).is_some()
    }
    pub(in crate::v2) fn iter(&self) -> Iter<'_, K, V> {
        let mut iter = Iter { stack: Vec::new() };
        iter.push_left(self.root.as_deref());
        iter
    }
    pub(in crate::v2) fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }
    pub(in crate::v2) fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}
impl<K: Ord + Clone, V: Clone> PMap<K, V> {
    pub(in crate::v2) fn insert(&mut self, key: K, val: V) -> Option<V> {
        insert(&mut self.root, key, val)
    }
    pub(in crate::v2) fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        remove(&mut self.root, key)
    }
    pub(in crate::v2) fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        // Changing a value through a shared tree copies the path first; a missing key must not
        // cost that copy.
        if !self.contains_key(key) {
            return None;
        }
        get_mut_in(&mut self.root, key)
    }
    pub(in crate::v2) fn clear(&mut self) {
        self.root = None;
    }
    pub(in crate::v2) fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        let kept = self
            .iter()
            .filter_map(|(k, v)| {
                let mut v = v.clone();
                keep(k, &mut v).then(|| (k.clone(), v))
            })
            .collect::<Vec<_>>();
        let count = kept.len();
        self.root = build(&mut kept.into_iter(), count);
    }
}
impl<K: Ord + Clone, V: Clone> Extend<(K, V)> for PMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, items: I) {
        for (key, val) in items {
            self.insert(key, val);
        }
    }
}
impl<K: Ord + Clone, V: Clone> FromIterator<(K, V)> for PMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(items: I) -> Self {
        // Later duplicates win, like repeated `insert`.
        let sorted = items
            .into_iter()
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .collect::<Vec<_>>();
        let count = sorted.len();
        PMap {
            root: build(&mut sorted.into_iter(), count),
        }
    }
}
impl<K: Ord + Clone, V: Clone> From<BTreeMap<K, V>> for PMap<K, V> {
    fn from(map: BTreeMap<K, V>) -> Self {
        map.into_iter().collect()
    }
}
impl<'a, K, V> IntoIterator for &'a PMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<K: serde::Serialize, V: serde::Serialize> serde::Serialize for PMap<K, V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}
impl<'de, K: serde::Deserialize<'de> + Ord + Clone, V: serde::Deserialize<'de> + Clone>
    serde::Deserialize<'de> for PMap<K, V>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::deserialize(deserializer).map(PMap::from)
    }
}

pub(in crate::v2) struct Iter<'a, K, V> {
    stack: Vec<&'a Node<K, V>>,
}
impl<'a, K, V> Iter<'a, K, V> {
    fn push_left(&mut self, mut node: Option<&'a Node<K, V>>) {
        while let Some(n) = node {
            self.stack.push(n);
            node = n.left.as_deref();
        }
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        self.push_left(node.right.as_deref());
        Some((&node.key, &node.val))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked<K: Ord + Clone + fmt::Debug, V: Clone + PartialEq + fmt::Debug>(
        tree: &PMap<K, V>,
        model: &BTreeMap<K, V>,
    ) {
        assert_eq!(tree.len(), model.len());
        let got = tree
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        let want = model
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        assert_eq!(got, want);
        // AVL invariant and cached sizes on every node
        fn walk<K, V>(link: &Link<K, V>) -> (u8, usize) {
            let Some(node) = link else { return (0, 0) };
            let (lh, ls) = walk(&node.left);
            let (rh, rs) = walk(&node.right);
            assert!((i16::from(lh) - i16::from(rh)).abs() <= 1, "unbalanced");
            assert_eq!(node.height, 1 + lh.max(rh));
            assert_eq!(node.size, 1 + ls + rs);
            (node.height, node.size)
        }
        walk(&tree.root);
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn behaves_like_a_btree_map_under_random_operations() {
        let mut rng = Rng(0x9e3779b97f4a7c15);
        let mut tree: PMap<String, u64> = PMap::default();
        let mut model: BTreeMap<String, u64> = BTreeMap::new();
        for step in 0..20_000u64 {
            let key = format!("k{}", rng.next() % 400);
            match rng.next() % 5 {
                0 | 1 => assert_eq!(tree.insert(key.clone(), step), model.insert(key, step)),
                2 => assert_eq!(tree.remove(key.as_str()), model.remove(&key)),
                3 => {
                    assert_eq!(tree.get(key.as_str()), model.get(&key));
                    if let (Some(a), Some(b)) = (tree.get_mut(key.as_str()), model.get_mut(&key)) {
                        *a += 1;
                        *b += 1;
                    }
                }
                _ => assert_eq!(tree.contains_key(key.as_str()), model.contains_key(&key)),
            }
            if step % 997 == 0 {
                checked(&tree, &model);
            }
        }
        checked(&tree, &model);
    }

    #[test]
    fn clones_are_independent_and_share_structure() {
        let mut base: PMap<u32, u32> = (0..1000).map(|i| (i, i)).collect();
        let snapshot = base.clone();
        for i in 0..1000 {
            *base.get_mut(&i).unwrap() = i + 1;
        }
        base.insert(5000, 5000);
        base.remove(&0);
        assert_eq!(snapshot.get(&0), Some(&0));
        assert_eq!(snapshot.get(&999), Some(&999));
        assert_eq!(snapshot.get(&5000), None);
        assert_eq!(snapshot.len(), 1000);
        assert_eq!(base.get(&999), Some(&1000));
        assert_eq!(base.get(&0), None);
        assert_eq!(base.len(), 1000);
        let model = (0..1000u32).map(|i| (i, i)).collect::<BTreeMap<_, _>>();
        checked(&snapshot, &model);
    }

    #[test]
    fn retain_extend_and_from_iter_match_the_model() {
        let mut tree: PMap<u32, u32> = (0..300).map(|i| (i * 3 % 301, i)).collect();
        let mut model: BTreeMap<u32, u32> = (0..300).map(|i| (i * 3 % 301, i)).collect();
        checked(&tree, &model);
        tree.retain(|k, v| {
            *v += 1;
            k % 2 == 0
        });
        model.retain(|k, v| {
            *v += 1;
            k % 2 == 0
        });
        checked(&tree, &model);
        tree.extend((1000..1100).map(|i| (i, i)));
        model.extend((1000..1100).map(|i| (i, i)));
        checked(&tree, &model);
        tree.clear();
        assert!(tree.is_empty());
    }

    #[test]
    fn a_large_table_can_be_branched_many_times_cheaply() {
        // 200k branches of a 50k-entry table: each branch changes one entry. With copy-on-write of
        // the whole table this would take minutes; with path copying it is a few hundred ms.
        let base: PMap<String, u32> = (0..50_000u32).map(|i| (format!("v{i}"), i)).collect();
        let started = std::time::Instant::now();
        for i in 0..200_000u32 {
            let mut branch = base.clone();
            branch.insert(format!("v{}", i % 50_000), i);
            assert_eq!(branch.len(), 50_000);
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
    }
}
