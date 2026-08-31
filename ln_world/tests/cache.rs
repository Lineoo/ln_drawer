use ln_world::{Element, HandleGeneric, ViewRef, World, WorldError};

#[derive(Debug, PartialEq, Eq)]
struct Tag(&'static str);
impl Element for Tag {}

#[test]
fn cache_invalidated_on_remove_cross_view() {
    let mut world = World::default();

    let view1 = world.insert(());
    let view2 = world.insert(());

    let node1a = world.enter(view1, || world.insert(Tag("node1a")));
    let node1b = world.enter(view1, || world.insert(Tag("node1b")));
    let node2 = world.enter(view2, || world.insert(Tag("node2")));

    world.enter(view2, || world.insert(ViewRef(view1.untyped())));
    world.flush();

    assert_eq!(tags(&world, view1), vec!["node1a", "node1b"]);
    assert_eq!(tags(&world, view2), vec!["node1a", "node1b", "node2"]);
    assert!(stags(&world, view1).is_err());
    assert!(stags(&world, view2).is_err());

    world.enter(view1, || world.queue_cache::<Tag>());
    world.flush();

    assert_eq!(tags(&world, view1), vec!["node1a", "node1b"]);
    assert_eq!(tags(&world, view2), vec!["node1a", "node1b", "node2"]);
    assert!(stags(&world, view1).is_err());
    assert!(stags(&world, view2).is_err());

    world.enter(view2, || world.remove(node1b).unwrap());
    world.flush();

    assert_eq!(tags(&world, view1), vec!["node1a"]);
    assert_eq!(tags(&world, view2), vec!["node1a", "node2"]);
    assert!(stags(&world, view1).is_ok());
    assert!(stags(&world, view2).is_err());

    assert!(world.enter(view1, || world.validate(node1a).is_ok()));
    assert!(world.enter(view1, || world.validate(node1b).is_err()));
    assert!(world.enter(view1, || world.validate(node2).is_err()));
    assert!(world.enter(view2, || world.validate(node1a).is_ok()));
    assert!(world.enter(view2, || world.validate(node1b).is_err()));
    assert!(world.enter(view2, || world.validate(node2).is_ok()));
}

/// Inserting a new element after a cache was built must not be missed by the
/// cache. Currently `cache::<T>()` is only invalidated on removal, so the newly
/// inserted element never shows up in `foreach`.
#[test]
fn cache_invalidated_on_insert() {
    let mut world = World::default();
    let view = world.insert(());

    world.enter(view, || world.insert(Tag("first")));
    world.flush();

    world.enter(view, || world.queue_cache::<Tag>());
    world.flush();
    assert_eq!(tags(&world, view), vec!["first"]);

    world.enter(view, || world.insert(Tag("second")));
    world.flush();

    assert_eq!(tags(&world, view), vec!["first", "second"]);
}

/// A `ViewRef`/`ElemRef` change alters what a view sees. Caches keyed on that
/// view must be dropped, otherwise `foreach` keeps yielding elements that are no
/// longer visible. Currently only the ref element's own type cache is dropped.
#[test]
fn cache_invalidated_on_viewref_change() {
    let mut world = World::default();
    let view1 = world.insert(());
    let view2 = world.insert(());

    let _node1 = world.enter(view1, || world.insert(Tag("v1")));
    let _node2 = world.enter(view2, || world.insert(Tag("v2")));
    let vref = world.enter(view2, || world.insert(ViewRef(view1.untyped())));
    world.flush();

    world.enter(view2, || world.queue_cache::<Tag>());
    world.flush();
    assert_eq!(tags(&world, view2), vec!["v1", "v2"]);

    world.enter(view2, || world.remove(vref).unwrap());
    world.flush();

    assert_eq!(tags(&world, view2), vec!["v2"]);
}

/// Inserting into a view that is visible from another view (via `ViewRef`) must
/// invalidate the other view's cache too, not just the insertion view's.
#[test]
fn cache_invalidated_on_insert_cross_view() {
    let mut world = World::default();
    let view1 = world.insert(());
    let view2 = world.insert(());

    world.enter(view2, || world.insert(ViewRef(view1.untyped())));
    world.enter(view1, || world.insert(Tag("first")));
    world.flush();

    world.enter(view2, || world.queue_cache::<Tag>());
    world.flush();
    assert_eq!(tags(&world, view2), vec!["first"]);

    world.enter(view1, || world.insert(Tag("second")));
    world.flush();

    assert_eq!(tags(&world, view2), vec!["first", "second"]);
}

fn tags(world: &World, view: impl HandleGeneric) -> Vec<&str> {
    let mut got = Vec::new();
    world.enter(view, || world.foreach_fetch::<Tag>(|h| got.push(h.0)));
    got.sort();
    got
}

fn stags(world: &World, view: impl HandleGeneric) -> Result<&str, WorldError> {
    Ok(world.enter(view, || world.single_fetch::<Tag>())?.0)
}
