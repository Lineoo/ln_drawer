use ln_world::*;

#[derive(Debug, PartialEq, Eq)]
struct TestBlanker;
impl Element for TestBlanker {}

#[derive(Debug, PartialEq, Eq)]
struct TestInserter(usize);
impl Element for TestInserter {}

#[derive(Debug, PartialEq, Eq)]
struct TestGoodInserter(usize);
impl Element for TestGoodInserter {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestEvent(usize);

#[test]
fn basic() {
    let mut world = World::default();

    let tester1h = world.insert(TestInserter(0xFC01));
    let tester2h = world.insert(TestInserter(0xFF02));
    let tester3h = world.insert(TestInserter(0xFB03));

    world.flush();

    let mut tester1 = world.fetch_mut(tester1h);
    let mut tester2 = world.fetch_mut(tester2h);
    let tester3 = world.fetch(tester3h);

    tester2.0 = 0xCC02;
    tester1.0 = tester3.0;

    drop(tester1);
    drop(tester2);
    drop(tester3);

    world.remove(tester3h);

    assert!(world.validate(tester3h).is_err());
    assert_eq!(world.fetch(tester1h).0, 0xFB03);
    assert_eq!(world.fetch(tester2h).0, 0xCC02);
}

#[test]
#[should_panic = "is mutably borrowed"]
fn runtime_borrow_panic() {
    let mut world = World::default();
    let tester1h = world.insert(TestInserter(0xFC01));
    world.flush();

    let _inserter1 = world.fetch_mut(tester1h);
    let _inserter2 = world.fetch(tester1h);
}

#[test]
fn loop_dependency() {
    let mut world = World::default();

    let left = world.insert(TestInserter(1));
    let right = world.insert(TestGoodInserter(2));
    let right_now = world.insert(TestGoodInserter(3));
    let but = world.insert(TestInserter(4));

    world.flush();

    world.dependency(left, right);
    world.dependency(right, left);
    world.dependency(right_now, right);

    world.remove(left);

    world.flush();

    assert!(world.validate(left).is_err());
    assert!(world.validate(right).is_err());
    assert!(world.validate(right_now).is_err());
    assert!(world.validate(but).is_ok());
}

#[test]
fn multi_dependency_children() {
    let mut world = World::default();

    let parent = world.insert(TestInserter(0));
    let child1 = world.insert(TestInserter(1));
    let child2 = world.insert(TestInserter(1));
    let child3 = world.insert(TestInserter(1));

    world.flush();

    world.dependency(child1, parent);
    world.dependency(child2, parent);
    world.dependency(child3, parent);

    world.remove(parent);

    world.flush();

    assert!(world.validate(parent).is_err());
    assert!(world.validate(child1).is_err());
    assert!(world.validate(child2).is_err());
    assert!(world.validate(child3).is_err());
}

#[test]
fn multi_dependency_parent() {
    let mut world = World::default();

    let child = world.insert(TestInserter(0));
    let child3 = world.insert(TestInserter(0));
    let parent1 = world.insert(TestInserter(1));
    let parent2 = world.insert(TestInserter(1));
    let parent3 = world.insert(TestInserter(1));

    world.flush();

    world.dependency(child, parent1);
    world.dependency(child, parent2);
    world.dependency(child, parent3);
    world.dependency(child3, parent3);

    world.remove(parent1);
    world.remove(parent3);

    world.flush();

    assert!(world.validate(child).is_err());
    assert!(world.validate(child3).is_err());
    assert!(world.validate(parent1).is_err());
    assert!(world.validate(parent2).is_ok());
    assert!(world.validate(parent3).is_err());
}

#[test]
fn multi_dependency_grand_parent() {
    let mut world = World::default();

    let grand_parent = world.insert(TestInserter(0));
    let parent = world.insert(TestInserter(1));
    let child = world.insert(TestInserter(2));

    world.flush();

    world.dependency(parent, grand_parent);
    world.dependency(child, parent);
    world.dependency(child, grand_parent);

    world.remove(grand_parent);

    world.flush();

    assert!(world.validate(grand_parent).is_err());
    assert!(world.validate(parent).is_err());
    assert!(world.validate(child).is_err());
}

#[test]
fn views() {
    let mut world = World::default();

    let view1 = world.insert(TestBlanker);
    let view2 = world.insert(TestBlanker);

    let node1 = world.enter(view1, || world.insert(TestInserter(1)));
    let node2 = world.enter(view2, || world.insert(TestInserter(2)));

    world.flush();

    assert!(world.enter(view1, || world.validate(node1).is_ok()));
    assert!(world.enter(view2, || world.validate(node2).is_ok()));
    assert!(world.enter(view1, || world.validate(node2).is_err()));
    assert!(world.enter(view2, || world.validate(node1).is_err()));
}

#[test]
fn view_refs_deps() {
    let mut world = World::default();

    let view1 = world.insert(TestBlanker);
    let view2 = world.insert(TestBlanker);
    let view3 = world.insert(TestBlanker);

    let node1 = world.enter(view1, || world.insert(TestInserter(1)));
    let node2 = world.enter(view2, || world.insert(TestInserter(2)));
    let node3 = world.enter(view3, || world.insert(TestInserter(3)));

    world.enter(view3, || world.dependency(node3, node1));

    world.enter(view2, || world.insert(ViewRef(view1.untyped())));
    world.enter(view3, || world.insert(ViewRef(view2.untyped())));

    world.flush();

    world.enter(view1, || world.remove(node1));

    world.flush();

    assert!(world.enter(view3, || world.validate(node1).is_err()));
    assert!(world.enter(view3, || world.validate(node2).is_ok()));
    assert!(world.enter(view3, || world.validate(node3).is_err()));
}

#[test]
fn view_refs_chain() {
    let mut world = World::default();

    let view1 = world.insert(TestBlanker);
    let view2 = world.insert(TestBlanker);
    let view3 = world.insert(TestBlanker);

    let node1 = world.enter(view1, || world.insert(TestInserter(1)));
    let node2 = world.enter(view2, || world.insert(TestInserter(2)));
    let node3 = world.enter(view3, || world.insert(TestInserter(3)));

    world.enter(view2, || world.insert(ViewRef(view1.untyped())));
    world.enter(view2, || world.insert(ViewRef(view3.untyped())));
    world.enter(view3, || world.insert(ViewRef(view2.untyped())));

    world.flush();

    assert!(world.enter(view3, || world.validate(node1).is_ok()));
    assert!(world.enter(view2, || world.validate(node1).is_ok()));
    assert!(world.enter(view1, || world.validate(node1).is_ok()));

    assert!(world.enter(view3, || world.validate(node2).is_ok()));
    assert!(world.enter(view2, || world.validate(node2).is_ok()));
    assert!(world.enter(view1, || world.validate(node2).is_err()));

    assert!(world.enter(view3, || world.validate(node3).is_ok()));
    assert!(world.enter(view2, || world.validate(node3).is_ok()));
    assert!(world.enter(view1, || world.validate(node3).is_err()));
}

#[test]
fn elem_refs_chain() {
    let mut world = World::default();

    let view1 = world.insert(TestBlanker);
    let view2 = world.insert(TestBlanker);
    let view3 = world.insert(TestBlanker);

    let node1a = world.enter(view1, || world.insert(TestInserter(11)));
    let node1b = world.enter(view1, || world.insert(TestInserter(12)));
    let node2 = world.enter(view2, || world.insert(TestInserter(2)));
    let node3 = world.enter(view3, || world.insert(TestInserter(3)));

    world.enter(view1, || world.insert(ViewRef(view2.untyped())));
    world.enter(view2, || world.insert(ElemRef(node1a.untyped())));
    world.enter(view2, || world.insert(ViewRef(view3.untyped())));
    world.enter(view3, || world.insert(ElemRef(node1b.untyped())));

    world.flush();

    assert!(world.enter(view1, || world.validate(node1a).is_ok()));
    assert!(world.enter(view1, || world.validate(node1b).is_ok()));
    assert!(world.enter(view1, || world.validate(node2).is_ok()));
    assert!(world.enter(view1, || world.validate(node3).is_ok()));

    assert!(world.enter(view2, || world.validate(node1a).is_ok()));
    assert!(world.enter(view2, || world.validate(node1b).is_ok()));
    assert!(world.enter(view2, || world.validate(node2).is_ok()));
    assert!(world.enter(view2, || world.validate(node3).is_ok()));

    assert!(world.enter(view3, || world.validate(node1a).is_err()));
    assert!(world.enter(view3, || world.validate(node1b).is_ok()));
    assert!(world.enter(view3, || world.validate(node2).is_err()));
    assert!(world.enter(view3, || world.validate(node3).is_ok()));
}

#[test]
fn observers() {
    let mut world = World::default();

    let left = world.insert(TestInserter(1));
    let right = world.insert(TestGoodInserter(2));

    world.flush();

    world.observer(left, move |TestEvent(i), world| {
        let mut this = world.fetch_mut(left);
        this.0 += i;
    });

    world.observer(left, move |TestEvent(i), world| {
        let mut this = world.fetch_mut(right);
        this.0 += i;
    });

    world.flush();

    world.trigger(left, &TestEvent(10));

    assert_eq!(&*world.fetch(left), &TestInserter(11));
    assert_eq!(&*world.fetch(right), &TestGoodInserter(12));
}

#[test]
fn generation_reuse() {
    let mut world = World::default();

    let old = world.insert(TestInserter(1));
    world.flush();

    world.remove(old);
    world.flush();

    assert!(world.validate(old).is_err());

    let new = world.insert(TestInserter(2));
    world.flush();

    assert!(world.validate(old).is_err());
    assert!(world.validate(new).is_ok());
    assert_eq!(&*world.fetch(new), &TestInserter(2));
}
