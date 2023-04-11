use crate::{
    context::Context, input::InputContext, render::RenderContext, time::TimeContext, window,
};
use winit::event_loop::EventLoop;

/// User callbaks
pub trait Callbacks {
    /// Called once per frame before render
    /// Return value determines wether to exit game or not
    fn update(&mut self, _ctx: &mut Context, _dt: f32) -> bool {
        false
    }
}

/// Config for initalizing window and game loop
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub resizeable: bool,
}

/// Main App
/// Contains all data to run application
pub struct App<C: Callbacks> {
    pub(crate) callbacks: C,
    pub(crate) config: Config,
}

/// Functions implemented on App
impl<C> App<C>
where
    C: Callbacks + 'static,
{
    /// Main loop which is called from window event loop
    /// Returns true if app should exit
    pub(crate) fn update(&mut self, ctx: &mut Context) -> bool {
        let dt = ctx.time.update_time();

        // Update callback
        if self.callbacks.update(ctx, dt) {
            return true;
        }

        ctx.input.keyboard.save_keys();

        false
    }
}

/// Runts event loop with callbacks
/// Calls back to user defined functions thorugh Callback trait
pub fn run<C: Callbacks + 'static>(callbacks: C, config: Config)
where
    C: Callbacks + 'static,
{
    env_logger::init();
    let app = App { callbacks, config };
    let (ctx, event_loop) = pollster::block_on(build_context(&app.config));
    pollster::block_on(window::run_window(event_loop, app, ctx));
}

// TODO contex builder?
pub async fn build_context(config: &Config) -> (Context, EventLoop<()>) {
    let (window, event_loop) = window::new_window(config);

    let time = TimeContext::default();
    let input = InputContext::default();
    let render = RenderContext::new(window, config).await;
    let context = Context {
        render,
        time,
        input,
    };

    (context, event_loop)
}
