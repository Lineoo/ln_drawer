use std::{cell::Cell, rc::Rc};

use ln_world::{ElemRef, Element, HandleGeneric, World};

#[derive(Debug, PartialEq, Eq)]
struct Tag(&'static str);
impl Element for Tag {}

struct Tick;

/// Guard: an observer registered on a cross-view target (visible only through an
/// `ElemRef`) must die together with its target.
///
/// This already holds today (`dependency` tolerates an invisible parent but still
/// records the edge), so it passes on current code. It exists to protect the
/// upcoming change that tightens `dependency` to require both endpoints visible:
/// that change must evaluate the observer's dependency from the observer's
/// registered view, or this lifecycle breaks.
#[test]
fn cross_view_observer_removed_with_target() {
    let mut world = World::default();
    let main = world.insert(());
    let ui = world.insert(());

    let target = world.enter(main, || world.insert(Tag("t")));
    world.enter(ui, || world.insert(ElemRef(target.untyped())));
    world.flush();

    let observer = world.enter(ui, || world.observer(target, move |_: &Tick, _: &World| {}));
    world.flush();

    world.enter(main, || world.remove(target).unwrap());
    world.flush();

    assert!(world.validate(observer).is_err());
}

/// Triggering an element that is invisible from the current view must not run
/// its observers. Currently `trigger` never validates the target, so observers
/// of a target in another (unreferenced) view are still executed.
#[test]
fn trigger_requires_target_visible() {
    let mut world = World::default();
    let view1 = world.insert(());
    let view2 = world.insert(());

    let target = world.enter(view1, || world.insert(Tag("t")));
    world.flush();

    let hits = Rc::new(Cell::new(0usize));
    let h = Rc::clone(&hits);
    world.enter(view1, || {
        world.observer(target, move |_: &Tick, _: &World| {
            h.set(h.get() + 1);
        });
    });
    world.flush();

    let cnt = world.enter(view2, || world.trigger(target, &Tick));
    assert_eq!(cnt, 0);
    assert_eq!(hits.get(), 0);
}
