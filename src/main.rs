extern crate ggez;
extern crate openmpt;
extern crate rodio;

use std::{
  fs::File,
  time::Duration,
};

use ggez::{conf, event, graphics, timer, input::keyboard, Context, GameResult};
use openmpt::module::{Module, Logger};
use rodio::{buffer::SamplesBuffer, Sink};

struct State {
  dt: Duration,
  module: Module,
  sink: Sink,
  buffer: Vec<f32>,
}

impl State {
  fn new() -> State {
    let sink = Sink::new(&rodio::default_output_device().unwrap());
    sink.pause();

    State {
      dt: Duration::default(),
      module: Module::create(
        &mut File::open("music/LPChip - Wisdom Of Purity.it").expect("open mod file"),
        Logger::None,
        &[]
      ).unwrap(),
      sink: sink,
      buffer: vec![0f32; 44100],
    }
  }
}

impl event::EventHandler for State {
  fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::set_window_title(ctx, "Upbeat");
    self.dt = timer::delta(ctx);

    if self.sink.len() < 2 {
      let mut avail_samples = self.module.read_interleaved_float_stereo(44100, &mut self.buffer);
      avail_samples = avail_samples << 1; // We're in interleaved stereo
      if avail_samples > 0 {
        let vec: Vec<f32> = self.buffer[..avail_samples].into();
        let samples_buffer = SamplesBuffer::new(2, 44100, vec);
        self.sink.append(samples_buffer);
      }
    }

    Ok(())
  }

  fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::clear(ctx, graphics::WHITE);
    graphics::present(ctx)
  }

  fn key_down_event(
    &mut self,
    ctx: &mut Context,
    keycode: keyboard::KeyCode,
    _keymods: keyboard::KeyMods,
    repeat: bool
  ) {
    if repeat { return; }

    match keycode {
      keyboard::KeyCode::Escape => event::quit(ctx),
      keyboard::KeyCode::Space => {
        if self.sink.is_paused() {
          self.sink.play();
        } else {
          self.sink.pause();
        }
      },
      _ => {}
    }
  }
}

fn main() {
  let c = conf::Conf::new();
  let (ref mut ctx, ref mut event_loop) = ggez::ContextBuilder::new("Upbeat", "David Simon")
    .conf(c)
    .build()
    .unwrap();

  let state = &mut State::new();
  event::run(ctx, event_loop, state).unwrap();
}
