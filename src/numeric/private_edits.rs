//! Private bounded kernel edits; callers refresh before returning any state.
use super::*;
use std::collections::HashSet;
// Writes keep arithmetic order, but refresh each touched page/path once when
// the private step finishes. Pointer keys are identities only: never dereferenced.
// No edited array is cloned or exposed between the first edit and finish.
pub(super) fn edit(
    node: &mut Arc<Node>,
    height: usize,
    index: usize,
    value: u64,
    dirty: &mut HashSet<usize>,
) {
    let node = Arc::make_mut(node);
    dirty.insert(node as *const Node as usize);
    match &mut node.kind {
        NodeKind::Leaf(bits) => bits[index] = value,
        NodeKind::Branch(left, right) => {
            let half = PAGE << (height - 1);
            if index < half {
                edit(left, height - 1, index, value, dirty);
            } else {
                edit(right, height - 1, index - half, value, dirty);
            }
        }
    }
}
pub(super) fn finish_edits(node: &mut Arc<Node>, dirty: &mut HashSet<usize>) {
    if !dirty.remove(&(Arc::as_ptr(node) as usize)) {
        return;
    }
    let node = Arc::get_mut(node).expect("private edited page unexpectedly shared");
    if let NodeKind::Branch(left, right) = &mut node.kind {
        finish_edits(left, dirty);
        finish_edits(right, dirty);
    }
    node.refresh();
}
