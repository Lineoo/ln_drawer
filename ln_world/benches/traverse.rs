#![feature(test)]

extern crate test;

use ln_world::{Element, World};
use test::Bencher;

struct Camera;
struct RenderControl;

impl Element for Camera {}
impl Element for RenderControl {}

#[bench]
fn traverse1x100(b: &mut Bencher) {
    let world = setup_world(1, 100);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse1x100_cached(b: &mut Bencher) {
    let world = setup_world_cached(1, 100);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse2x100(b: &mut Bencher) {
    let world = setup_world(2, 100);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse2x100_cached(b: &mut Bencher) {
    let world = setup_world_cached(2, 100);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse5x100(b: &mut Bencher) {
    let world = setup_world(5, 100);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse5x100_cached(b: &mut Bencher) {
    let world = setup_world_cached(5, 100);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse2x50(b: &mut Bencher) {
    let world = setup_world(2, 50);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse2x50_cached(b: &mut Bencher) {
    let world = setup_world_cached(2, 50);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse5x20(b: &mut Bencher) {
    let world = setup_world(5, 20);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse5x20_cached(b: &mut Bencher) {
    let world = setup_world_cached(5, 20);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse10x10(b: &mut Bencher) {
    let world = setup_world(10, 10);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse10x10_cached(b: &mut Bencher) {
    let world = setup_world_cached(10, 10);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse50x2(b: &mut Bencher) {
    let world = setup_world(50, 2);
    b.iter(|| traverse_control(&world));
}

#[bench]
fn traverse50x2_cached(b: &mut Bencher) {
    let world = setup_world_cached(50, 2);
    b.iter(|| traverse_control(&world));
}

fn traverse_control(world: &World) {
    world.foreach_enter::<Camera>(|camera| {
        world.foreach_fetch::<RenderControl>(|control| {
            test::black_box((camera, control));
        });
    });
}

fn setup_world(camera: usize, control: usize) -> World {
    let mut world = World::new();

    for _ in 0..camera {
        let camera = world.insert(Camera);
        world.enter(camera, || {
            for _ in 0..control {
                world.insert(RenderControl);
            }
        });
    }

    // setup
    world.flush();
    world
}

fn setup_world_cached(camera: usize, control: usize) -> World {
    let mut world = World::new();

    for _ in 0..camera {
        let camera = world.insert(Camera);
        world.enter(camera, || {
            for _ in 0..control {
                world.insert(RenderControl);
            }
            world.queue_cache::<RenderControl>();
        });
    }

    // setup
    world.queue_cache::<Camera>();
    world.flush();
    world
}
