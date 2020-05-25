fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 1024,
        height: 1024,
        parent: baseview::Parent::None,
    };

    baseview::run(window_open_options);
}
