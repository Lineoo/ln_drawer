use crate::{
    animation::{AnimationDescriptor, SimpleAnimationDescriptor},
    layout::{
        LayoutControl, LayoutControls,
        transform::{Transform, TransformValue},
    },
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    theme::Luni,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        WidgetAnimatedRectangle, WidgetButton, WidgetClick, WidgetDestroyed, WidgetExpanded,
        WidgetHover, WidgetRectangle,
    },
    world::{Element, Handle, World},
};

pub struct Expandable {
    pub rect: Rectangle,
    pub transform: TransformValue,
    pub expanded: bool,
}

impl Expandable {
    fn receive_layout(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(LayoutControl {
            rectangle: Some(Box::new(move |world, rect| {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = rect;
                if this.expanded {
                    let rect = this.transform.compute(rect);
                    world.queue_trigger(this.handle(), WidgetRectangle(rect));
                } else {
                    world.queue_trigger(this.handle(), WidgetRectangle(rect));
                }

                rect
            })),
            enabled: None,
        });

        let mut layouts = world.single_fetch_mut::<LayoutControls>().unwrap();
        layouts.0.insert(this.untyped(), control);
        world.dependency(control, this);
    }

    fn attach_luni(&mut self, world: &World, this: Handle<Self>) {
        let luni = world.single_fetch::<Luni>().unwrap();

        let frame = world.build(RoundedRectDescriptor {
            rect: self.rect,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            visible: true,
            order: 10,
        });

        let frame_rect = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(
                [
                    self.rect.left() as f32,
                    self.rect.down() as f32,
                    self.rect.right() as f32,
                    self.rect.up() as f32,
                ],
                luni.anim_factor,
            ),
            widget: frame,
            action: |mut frame, _, rect| {
                frame.desc.rect = Rectangle::new(
                    rect[0].round() as i32,
                    rect[1].round() as i32,
                    rect[2].round() as i32,
                    rect[3].round() as i32,
                );
            },
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut frame_rect = world.fetch_mut(frame_rect).unwrap();
            let rect = [
                rect.left() as f32,
                rect.down() as f32,
                rect.right() as f32,
                rect.up() as f32,
            ];

            frame_rect.src = rect;
            frame_rect.dst = rect;
        });

        world.observer(this, move |&WidgetAnimatedRectangle(rect), world| {
            let mut frame_rect = world.fetch_mut(frame_rect).unwrap();
            let rect = [
                rect.left() as f32,
                rect.down() as f32,
                rect.right() as f32,
                rect.up() as f32,
            ];

            frame_rect.dst = rect;
        });

        world.observer(this, move |&WidgetDestroyed, world| {
            world.remove(frame).unwrap();
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 10,
            enabled: true,
        });

        world.insert(Transform {
            value: TransformValue::copy(),
            source: this.untyped(),
            target: collider.untyped(),
        });

        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world| {
            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                }
                PointerHitStatus::Release => {
                    world.trigger(this, &WidgetClick);
                    world.trigger(this, &WidgetButton::ButtonRelease);

                    let mut this = world.fetch_mut(this).unwrap();
                    this.expanded = !this.expanded;
                    if this.expanded {
                        let rect = this.transform.compute(this.rect);
                        world.queue_trigger(this.handle(), WidgetAnimatedRectangle(rect));
                        world.queue_trigger(this.handle(), WidgetExpanded(true));
                    } else {
                        world.queue_trigger(this.handle(), WidgetAnimatedRectangle(this.rect));
                        world.queue_trigger(this.handle(), WidgetExpanded(false));
                    }
                }
                _ => {}
            }
        });

        world.observer(collider, move |event: &PointerHover, world| {
            match event.status {
                PointerHoverStatus::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerHoverStatus::Leave => {
                    world.trigger(this, &WidgetHover::HoverLeave);
                }
                _ => {}
            }
        });
    }
}

impl Element for Expandable {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.receive_layout(world, this);
        self.attach_luni(world, this);
        self.attach_pointer(world, this);
    }
}
