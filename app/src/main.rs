use crate::app::App;

mod app;
mod ctx;
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
