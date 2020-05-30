# baseview

A low-level windowing system geared towards making audio plugin UIs.

`baseview` abstracts the platform-specific windowing APIs (winapi, cocoa, xcb) into a platform-independent API, but otherwise gets out of your way so you can write plugin UIs.

## Roadmap

Below is a proposed list of milestones (roughly in-order) and their status. Subject to change at any time.

| Feature                                | Windows            | Mac OS             | Linux             |
| -------------------------------------- | ------------------ | ------------------ | ----------------- |
| Spawns a window, no parent             | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark:|
| Cross-platform API for window spawning | :heavy_check_mark: | :heavy_check_mark: | :heavy_check_mark:|
| Window uses an OpenGL surface          | :heavy_check_mark: |                    |                   |
| Basic DPI scaling support              |                    |                    | :question:        |
