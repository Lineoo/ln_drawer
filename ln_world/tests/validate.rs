use ln_world::{ElemRef, Element, ViewRef, World};

#[derive(Debug, PartialEq, Eq)]
struct Tag(&'static str);
impl Element for Tag {}

/// A dangling view/element reference in `elemrefs`/`viewrefs` must not abort the
/// visibility BFS when an alternative path to the current view still exists.
///
/// Construction: insert an `ElemRef` into a view, then remove that view before
/// `flush`. The ref's queued bookkeeping runs after the view index is gone, so
/// the target's `elemrefs` keeps a handle to the removed view. Validating the
/// target from a view that can still reach it through a `ViewRef` then hits the
/// dangling handle first and currently errors out with `InvalidHandle` instead
/// of continuing the search.
#[test]
fn dangling_ref_does_not_break_alternative_visibility() {
    let mut world = World::default();
    let view_remove = world.insert(());
    let view_main = world.insert(());
    let view_other = world.insert(());

    let node = world.enter(view_main, || world.insert(Tag("x")));
    world.flush();

    world.enter(view_remove, || world.insert(ElemRef(node.untyped())));
    world.remove(view_remove).unwrap();
    world.flush();

    world.enter(view_other, || world.insert(ViewRef(view_main.untyped())));
    world.flush();

    assert!(world.enter(view_other, || world.validate(node).is_ok()));
}
