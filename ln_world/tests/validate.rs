use ln_world::{ElemRef, Element, Handle, ViewRef, World};

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

/// Guard: cached iteration must not observe elements that were removed but not
/// yet flushed.
///
/// Currently FAILS: the cache-hit paths skip validation and `remove` only
/// invalidates the cache from the queued command, so between `remove` and
/// `flush` the just-removed element is still yielded. This asserts the desired
/// behaviour and turns green once the cache becomes strictly valid.
#[test]
fn cache_does_not_see_just_removed_before_flush() {
    let mut world = World::default();
    let view = world.insert(());

    let first = world.enter(view, || world.insert(Tag("first")));
    world.enter(view, || world.insert(Tag("second")));
    world.flush();

    world.enter(view, || world.queue_cache::<Tag>());
    world.flush();

    world.enter(view, || world.remove(first).unwrap());

    assert_eq!(tags(&world, view), vec!["second"]);

    world.flush();

    assert_eq!(tags(&world, view), vec!["second"]);
}

/// Guard: `single_fetch` must not hand out a live reference to a removed element.
///
/// Currently FAILS: `single` trusts the stale cache and `single_fetch` goes
/// through `fetch_unchecked` (which skips validation and the `removed` set), so
/// it returns a `Ref` into the just-removed element instead of an error.
#[test]
fn single_fetch_returns_just_removed_before_flush() {
    let mut world = World::default();
    let view = world.insert(());

    let tag = world.enter(view, || world.insert(Tag("first")));
    world.flush();

    world.enter(view, || world.queue_cache::<Tag>());
    world.flush();

    world.enter(view, || world.remove(tag).unwrap());

    assert!(world.enter(view, || world.single_fetch::<Tag>()).is_err());
}

fn tags(world: &World, view: Handle<impl ?Sized>) -> Vec<&str> {
    let mut got = Vec::new();
    world.enter(view, || world.foreach_fetch::<Tag>(|h| got.push(h.0)));
    got.sort();
    got
}
