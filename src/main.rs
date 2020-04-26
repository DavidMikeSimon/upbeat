extern crate ggez;
extern crate openmpt;
extern crate rodio;

use std::{
  fs::File,
  thread,
  time::Duration,
};

use ggez::{conf, event, graphics, timer, Context, GameResult};
use openmpt::module::{Module, Logger};
use rodio::{buffer::SamplesBuffer, Sink};

struct State {
  dt: Duration,
}

impl Default for State {
  fn default() -> State {
    State {
      dt: Duration::default(),
    }
  }
}

impl event::EventHandler for State {
  fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
    self.dt = timer::delta(ctx);
    Ok(())
  }
  fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
    Ok(())
  }
}

fn main() {
  let mut stream = File::open("music/LPChip - Wisdom Of Purity.it").expect("open mod file");

	let mut module = Module::create(&mut stream, Logger::None, &[]).unwrap();

  let device = rodio::default_output_device().unwrap();
  let sink = Sink::new(&device);
  sink.pause();

	let mut buffer = vec![0f32; 44100];

	loop {
		let avail_samples = module.read_interleaved_float_stereo(
				44100, &mut buffer) << 1; // We're in interleaved stereo
		if avail_samples <= 0 { break; }

    let vec: Vec<f32> = buffer[..avail_samples].into();
    let buffer = SamplesBuffer::new(2, 44100, vec);
    sink.append(buffer);
	}

  let state = &mut State::default();

  let c = conf::Conf::new();
  let (ref mut ctx, ref mut event_loop) = ggez::ContextBuilder::new("Upbeat", "David Simon")
    .conf(c)
    .build()
    .unwrap();

  event::run(ctx, event_loop, state).unwrap();
}
