### general, val - dont DRY too hard

DO NOT:

```rs
set_val(world, slider.value1, 1.0);
set_val(world, slider.value2, 2.0);

fn set_val(world: &World, value: Handle<Val<f32>>, val: f32) {
    world.fetch_mut(value).val = val;
    world.queue_trigger(value, ValChanged);
}
```

DO:

```rs
world.fetch_mut(slider.value1).val = 1.0;
world.fetch_mut(slider.value2).val = 2.0;
world.queue_trigger(slider.value1, ValChanged);
world.queue_trigger(slider.value2, ValChanged);
```

### observer - re-fetch handle

DO NOT:

```rs
let theme = world.single_fetch::<Theme>();
let visible = self.common.visible;
let grey0 = theme.grey0;
world.observer(this, move || {
    let visible = world.fetch(visible).val;
    let grey0 = world.fetch(grey0).val;
})
```

DO:

```rs
world.observer(this, move || {
    let this = world.fetch(this);
    let theme = world.single_fetch::<Theme>();
    let visible = world.fetch(this.common.visible).val;
    let grey0 = world.fetch(theme.grey0).val;
})
```

### ln_world - no try_fetch

DO NOT:

```rs
world.try_fetch(handle)
    .map(|x| x.val)
    .unwrap_or_default();
```

DO:

```rs
world.fetch(handle).val;
```

### ln_world - do not overuse blocks and drop

DO NOT:

```rs
let scale = {
    let lnwindow = world.single_fetch::<Lnwindow>();
    lnwindow.window.scale_factor()
};
```

DO NOT:

```rs
let lnwindow = world.single_fetch::<Lnwindow>();
let scale = lnwindow.window.scale_factor();
drop(lnwindow);
```

DO:

```rs
let lnwindow = world.single_fetch::<Lnwindow>();
let scale = lnwindow.window.scale_factor();
```

The command queue will handle most reference collisions. If `drop(lnwindow)` is unavoidable, it means either that your implementation touches the danger zone (tell users about it), or you've implement it in a wrong way (think twice).
