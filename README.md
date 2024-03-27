# baseview

A low-level windowing system geared towards making audio plugin UIs.

`baseview` abstracts the platform-specific windowing APIs (winapi, cocoa, xcb) into a platform-independent API, but otherwise gets out of your way so you can write plugin UIs.

Interested in learning more about the project? Join us on [discord](https://discord.gg/b3hjnGw), channel `#plugin-gui`.

## Prerequisites

### Linux

Install dependencies, e.g.,

```sh
sudo apt-get install libx11-dev libxcursor-dev libxcb-dri2-0-dev libxcb-icccm4-dev libx11-xcb-dev
```

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Baseview by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
