use crate::app::App;

mod app;
mod camera;
mod ctx;
mod entity;
mod net;
mod scene;
mod stack;

fn main() {
    let mut app = App::new();
    while !app.should_close() {
        app.update();
        app.draw();
    }
}
