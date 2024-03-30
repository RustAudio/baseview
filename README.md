# baseview

A low-level windowing system geared towards making audio plugin UIs.

`baseview` abstracts the platform-specific windowing APIs (winapi, cocoa, xcb) into a platform-independent API, but otherwise gets out of your way so you can write plugin UIs.

Interested in learning more about the project? Join us on [discord](https://discord.gg/b3hjnGw), channel `#plugin-gui`.

## Roadmap

Below is a proposed list of milestones (roughly in-order) and their status. Subject to change at any time.

| Feature                                               | Windows            | Mac OS             | Linux              |
| ----------------------------------------------------- | ------------------ | ------------------ | ------------------ |
| Spawns a window, no parent                            | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark: |
| Cross-platform API for window spawning                | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark: |
| Can find DPI scale factor                             |                    | :heavy_check_mark: | :heavy_check_mark: |
| Basic event handling (mouse, keyboard)                | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark: |
| Parent window support                                 | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark: |
| OpenGL context creation (behind the `opengl` feature) | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark: |

## Prerequisites

### Linux

Install dependencies, e.g.:

```sh
sudo apt-get install libx11-dev libxcb1-dev libx11-xcb-dev libgl1-mesa-dev
```

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Baseview by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
