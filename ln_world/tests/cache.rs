use ln_world::{Element, Handle, ViewRef, World, WorldError};

#[derive(Debug, PartialEq, Eq)]
struct Tag(&'static str);
impl Element for Tag {}

#[test]
fn validate_cache_invalidated_on_remove() {
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

fn tags(world: &World, view: Handle<impl ?Sized>) -> Vec<&str> {
    let mut got = Vec::new();
    world.enter(view, || world.foreach_fetch::<Tag>(|h| got.push(h.0)));
    got.sort();
    got
}

fn stags(world: &World, view: Handle<impl ?Sized>) -> Result<&str, WorldError> {
    Ok(world.enter(view, || world.single_fetch::<Tag>())?.0)
}
