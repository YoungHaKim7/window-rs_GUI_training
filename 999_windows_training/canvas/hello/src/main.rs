#![windows_subsystem = "windows"]

use windows_canvas::{ColorF, DrawContext, Ellipse as CanvasEllipse, Result, Vector2, canvas};
use windows_reactor::{
    App, Component, ComponentContext, View, ViewContext, WindowBackdrop, WindowVisuals,
};

fn draw(ctx: &DrawContext) -> Result<()> {
    ctx.clear(ColorF::DARK_SLATE_BLUE);
    let brush = ctx.create_solid_brush(ColorF::CORNFLOWER_BLUE)?;
    let r = ctx.width.min(ctx.height) * 0.3;
    ctx.fill_ellipse(
        &CanvasEllipse::circle(Vector2::new(ctx.width / 2.0, ctx.height / 2.0), r),
        &brush,
    );
    Ok(())
}

struct Sample;

impl Component for Sample {
    type Input = ();
    type Message = ();

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        Self
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title("Hello Canvas");
        context.window_visuals(WindowVisuals::new().backdrop(WindowBackdrop::Mica));
        canvas(draw)
    }
}

fn main() -> Result<()> {
    App::run_component::<Sample>(())
}
