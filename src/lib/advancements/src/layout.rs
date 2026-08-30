//! Where each advancement sits on its tree.
//!
//! The client draws them where it is told, so the server works the layout out. This is vanilla's
//! `TreeNodePosition` — the Reingold–Tilford tree layout, in Buchheim's linear form — turned from
//! a graph of object references into an arena of indices, which is the same algorithm and the same
//! result.
//!
//! Depth runs along x and siblings along y, which is the way the advancement screen reads.

use crate::Advancement;
use std::collections::BTreeMap;

#[derive(Default)]
struct Node {
    name: String,
    parent: Option<usize>,
    previous_sibling: Option<usize>,
    /// Which child of its parent this is, counting from one, which the shifting divides by.
    child_index: i32,
    children: Vec<usize>,
    /// The subtree this one's shifts are measured against.
    ancestor: usize,
    /// A shortcut across a subtree's contour, so walking one costs its edge rather than its size.
    thread: Option<usize>,
    x: i32,
    y: f32,
    offset: f32,
    change: f32,
    shift: f32,
}

/// Works out where every visible advancement sits.
///
/// Each root with something to show is laid out on its own, since each is its own tab.
#[must_use]
pub fn lay_out(advancements: &BTreeMap<String, Advancement>) -> BTreeMap<String, (f32, f32)> {
    // Who hangs off whom. An advancement whose parent is not there cannot be placed.
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut roots = Vec::new();
    for (name, advancement) in advancements {
        match &advancement.parent {
            Some(parent) if advancements.contains_key(parent.as_str()) => {
                children.entry(parent.as_str()).or_default().push(name);
            }
            _ => roots.push(name.as_str()),
        }
    }

    let mut positions = BTreeMap::new();
    for root in roots {
        // A root with nothing to show has nothing to lay out, and neither do its children: the
        // recipe tree is one such, and the client never draws it.
        if advancements
            .get(root)
            .is_none_or(|advancement| advancement.display.is_none())
        {
            continue;
        }
        let mut arena = Vec::new();
        let root_node = build(&mut arena, advancements, &children, root, None, None, 1, 0);
        first_walk(&mut arena, root_node);
        let start = arena[root_node].y;
        let lowest = second_walk(&mut arena, root_node, 0.0, 0, start);
        if lowest < 0.0 {
            third_walk(&mut arena, root_node, -lowest);
        }
        for node in &arena {
            positions.insert(node.name.clone(), (node.x as f32, node.y));
        }
    }
    positions
}

/// Builds the node for one advancement and everything visible below it.
#[expect(clippy::too_many_arguments)]
fn build(
    arena: &mut Vec<Node>,
    advancements: &BTreeMap<String, Advancement>,
    children: &BTreeMap<&str, Vec<&str>>,
    name: &str,
    parent: Option<usize>,
    previous_sibling: Option<usize>,
    child_index: i32,
    depth: i32,
) -> usize {
    let index = arena.len();
    arena.push(Node {
        name: name.to_owned(),
        parent,
        previous_sibling,
        child_index,
        ancestor: index,
        x: depth,
        y: -1.0,
        ..Node::default()
    });

    let mut previous = None;
    for child in children.get(name).map(Vec::as_slice).unwrap_or_default() {
        previous = add_child(arena, advancements, children, child, index, previous, depth);
    }
    index
}

/// Adds a child, or its own children where it is invisible: an advancement with nothing to show is
/// not drawn, and whatever hangs off it hangs off its parent instead.
fn add_child(
    arena: &mut Vec<Node>,
    advancements: &BTreeMap<String, Advancement>,
    children: &BTreeMap<&str, Vec<&str>>,
    name: &str,
    parent: usize,
    previous: Option<usize>,
    depth: i32,
) -> Option<usize> {
    if advancements
        .get(name)
        .is_some_and(|advancement| advancement.display.is_some())
    {
        let child_index = arena[parent].children.len() as i32 + 1;
        let node = build(
            arena,
            advancements,
            children,
            name,
            Some(parent),
            previous,
            child_index,
            depth + 1,
        );
        arena[parent].children.push(node);
        Some(node)
    } else {
        let mut previous = previous;
        for grandchild in children.get(name).map(Vec::as_slice).unwrap_or_default() {
            previous = add_child(
                arena,
                advancements,
                children,
                grandchild,
                parent,
                previous,
                depth,
            );
        }
        previous
    }
}

/// Gives every node a place among its siblings, pushing subtrees apart where they would collide.
fn first_walk(arena: &mut [Node], index: usize) {
    if arena[index].children.is_empty() {
        arena[index].y = match arena[index].previous_sibling {
            Some(previous) => arena[previous].y + 1.0,
            None => 0.0,
        };
        return;
    }

    let mut default_ancestor = None;
    for at in 0..arena[index].children.len() {
        let child = arena[index].children[at];
        first_walk(arena, child);
        default_ancestor = Some(apportion(arena, child, default_ancestor.unwrap_or(child)));
    }
    execute_shifts(arena, index);

    let first = arena[index].children[0];
    let last = arena[index].children[arena[index].children.len() - 1];
    let midpoint = (arena[first].y + arena[last].y) / 2.0;
    match arena[index].previous_sibling {
        Some(previous) => {
            arena[index].y = arena[previous].y + 1.0;
            arena[index].offset = arena[index].y - midpoint;
        }
        None => arena[index].y = midpoint,
    }
}

/// Adds up the offsets down each branch, and reports the highest place anything ended up in.
fn second_walk(arena: &mut [Node], index: usize, offset: f32, depth: i32, mut lowest: f32) -> f32 {
    arena[index].y += offset;
    arena[index].x = depth;
    lowest = lowest.min(arena[index].y);
    let offset = offset + arena[index].offset;
    for at in 0..arena[index].children.len() {
        let child = arena[index].children[at];
        lowest = second_walk(arena, child, offset, depth + 1, lowest);
    }
    lowest
}

/// Slides the whole tree down so nothing sits above the top.
fn third_walk(arena: &mut [Node], index: usize, offset: f32) {
    arena[index].y += offset;
    for at in 0..arena[index].children.len() {
        let child = arena[index].children[at];
        third_walk(arena, child, offset);
    }
}

/// Applies the shifts a round of apportioning asked for, from the last child back.
fn execute_shifts(arena: &mut [Node], index: usize) {
    let mut shift = 0.0;
    let mut change = 0.0;
    for at in (0..arena[index].children.len()).rev() {
        let child = arena[index].children[at];
        arena[child].y += shift;
        arena[child].offset += shift;
        change += arena[child].change;
        shift += arena[child].shift + change;
    }
}

/// The next node along a subtree's left contour, following a thread where one was laid.
fn previous_or_thread(arena: &[Node], index: usize) -> Option<usize> {
    arena[index]
        .thread
        .or_else(|| arena[index].children.first().copied())
}

/// The same along the right contour.
fn next_or_thread(arena: &[Node], index: usize) -> Option<usize> {
    arena[index]
        .thread
        .or_else(|| arena[index].children.last().copied())
}

/// Pushes this subtree far enough right of its left neighbour that the two do not overlap.
fn apportion(arena: &mut [Node], index: usize, default_ancestor: usize) -> usize {
    let Some(previous_sibling) = arena[index].previous_sibling else {
        return default_ancestor;
    };
    let Some(parent) = arena[index].parent else {
        return default_ancestor;
    };
    let mut default_ancestor = default_ancestor;

    let (mut inside_right, mut outside_right) = (index, index);
    let mut inside_left = previous_sibling;
    let mut outside_left = arena[parent].children[0];
    let mut shift_inside_right = arena[index].offset;
    let mut shift_outside_right = arena[index].offset;
    let mut shift_inside_left = arena[inside_left].offset;
    let mut shift_outside_left = arena[outside_left].offset;

    while let (Some(next_left), Some(previous_right)) = (
        next_or_thread(arena, inside_left),
        previous_or_thread(arena, inside_right),
    ) {
        inside_left = next_left;
        inside_right = previous_right;
        outside_left = previous_or_thread(arena, outside_left).unwrap_or(outside_left);
        outside_right = next_or_thread(arena, outside_right).unwrap_or(outside_right);
        arena[outside_right].ancestor = index;

        let overlap = arena[inside_left].y + shift_inside_left
            - (arena[inside_right].y + shift_inside_right)
            + 1.0;
        if overlap > 0.0 {
            let ancestor = ancestor_of(arena, inside_left, index, default_ancestor);
            move_subtree(arena, ancestor, index, overlap);
            shift_inside_right += overlap;
            shift_outside_right += overlap;
        }

        shift_inside_left += arena[inside_left].offset;
        shift_inside_right += arena[inside_right].offset;
        shift_outside_left += arena[outside_left].offset;
        shift_outside_right += arena[outside_right].offset;
    }

    if next_or_thread(arena, inside_left).is_some()
        && next_or_thread(arena, outside_right).is_none()
    {
        arena[outside_right].thread = next_or_thread(arena, inside_left);
        arena[outside_right].offset += shift_inside_left - shift_outside_right;
    } else {
        if previous_or_thread(arena, inside_right).is_some()
            && previous_or_thread(arena, outside_left).is_none()
        {
            arena[outside_left].thread = previous_or_thread(arena, inside_right);
            arena[outside_left].offset += shift_inside_right - shift_outside_left;
        }
        default_ancestor = index;
    }
    default_ancestor
}

/// Spreads a shift over the subtrees between two siblings, so the ones in between move by their
/// share rather than all at once.
fn move_subtree(arena: &mut [Node], left: usize, right: usize, shift: f32) {
    let between = arena[right].child_index - arena[left].child_index;
    if between != 0 {
        let each = shift / between as f32;
        arena[right].change -= each;
        arena[left].change += each;
    }
    arena[right].shift += shift;
    arena[right].y += shift;
    arena[right].offset += shift;
}

/// The subtree a shift should be measured against: the recorded one where it is a sibling of the
/// node being placed, and the fallback otherwise.
fn ancestor_of(arena: &[Node], index: usize, other: usize, default_ancestor: usize) -> usize {
    let recorded = arena[index].ancestor;
    match arena[other].parent {
        Some(parent) if arena[parent].children.contains(&recorded) => recorded,
        _ => default_ancestor,
    }
}
