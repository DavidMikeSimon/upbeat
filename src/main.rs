extern crate ggez;
extern crate nalgebra;
extern crate openmpt;
extern crate rodio;

use std::{
  convert::TryInto,
  fs::File,
  time::Duration,
  iter
};

use ggez::{conf, event, graphics, timer, input::keyboard, Context, GameResult};
use openmpt::{
  module::{Module, Logger},
  mod_command::{Note}
};
use rodio::{buffer::SamplesBuffer, Sink};

const SAMPLES_PER_SEC: usize = 44100;
const BUFFER_LEN: usize = SAMPLES_PER_SEC/4;

#[derive(Copy, Clone, Debug)]
enum RelativePitch {
  High,
  Low
}

struct PitchInfo {
  pitch: u8,
  relative_pitch: RelativePitch,
}

fn get_pattern(module: &mut Module) -> Vec<Vec<Option<PitchInfo>>> {
  dbg!(module.get_num_patterns());
  dbg!(module.get_num_orders());
  dbg!(module.get_num_channels());
  dbg!(module.get_num_instruments());
  dbg!(module.get_num_samples());

  let num_orders: usize = module.get_num_orders().try_into().unwrap();
  let num_channels: usize = module.get_num_channels().try_into().unwrap();

  let mut r_pattern = Vec::new();

  let mut prior_pitch: Vec<u8> = iter::repeat(0).take(num_channels).collect();
  let mut prior_relative_pitch: Vec<RelativePitch> = iter::repeat(RelativePitch::High).take(num_channels).collect();

  for order_num in 0..num_orders {
    let mut pattern = module.get_pattern_by_order(order_num.try_into().unwrap()).unwrap();
    let num_rows = pattern.get_num_rows();
    for row_num in 0..num_rows {
      let mut row_pattern = Vec::new();
      let mut row = pattern.get_row_by_number(row_num).unwrap();
      for channel_num in 0..num_channels {
        let mut cell = row.get_cell_by_channel(channel_num.try_into().unwrap()).unwrap();
        if let Ok(mod_command) = cell.get_data() {
          match mod_command.note {
            Note::Note(pitch) => {
              let relative_pitch = if pitch == prior_pitch[channel_num] {
                prior_relative_pitch[channel_num]
              } else if pitch > prior_pitch[channel_num] {
                RelativePitch::High
              } else {
                RelativePitch::Low
              };
              row_pattern.push(Some(PitchInfo {
                pitch: pitch,
                relative_pitch: relative_pitch,
              }));
              prior_relative_pitch[channel_num] = relative_pitch;
              prior_pitch[channel_num] = pitch;
            }
            _ => row_pattern.push(None)
          }
        }
      }
  
      r_pattern.push(row_pattern);
    }
  }

  r_pattern
}

struct State {
  dt: Duration,
  play_offset: Duration,
  module: Module,
  module_duration: f64,
  pattern: Vec<Vec<Option<PitchInfo>>>,
  sink: Sink,
  buffer: Vec<f32>,
}

impl State {
  fn new() -> State {
    let mut module = Module::create(
      &mut File::open("music/weeppiko_musix_-_were_fighting_again.mptm").expect("open mod file"),
      // &mut File::open("music/LPChip - Wisdom Of Purity.it").expect("open mod file"),
      Logger::None,
      &[]
    ).unwrap();
    let module_duration = module.get_duration_seconds();

    let pattern = get_pattern(&mut module);

    let sink = Sink::new(&rodio::default_output_device().unwrap());
    sink.pause();

    State {
      dt: Duration::default(),
      play_offset: Duration::default(),
      module: module,
      module_duration: module_duration,
      pattern: pattern,
      sink: sink,
      buffer: vec![0f32; BUFFER_LEN],
    }
  }
}

impl event::EventHandler for State {
  fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::set_window_title(ctx, "Upbeat");
    self.dt = timer::delta(ctx);

    if !self.sink.is_paused() {
      self.play_offset += self.dt;
    }

    if self.sink.len() < 2 {
      let mut avail_samples = self.module.read_interleaved_float_stereo(SAMPLES_PER_SEC as i32, &mut self.buffer);
      avail_samples = avail_samples << 1; // We're in interleaved stereo
      if avail_samples > 0 {
        let vec: Vec<f32> = self.buffer[..avail_samples].into();
        let samples_buffer = SamplesBuffer::new(2, SAMPLES_PER_SEC as u32, vec);
        self.sink.append(samples_buffer);
      }
    }

    Ok(())
  }

  fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::clear(ctx, graphics::WHITE);

    let window = graphics::screen_coordinates(ctx);

    let now_line_x = 200.0;
    let now_line_width = 2.0;

    let line_mesh = graphics::Mesh::new_line(
      ctx,
      &[
        nalgebra::Point2::new(now_line_x + now_line_width/2.0, 0.0),
        nalgebra::Point2::new(now_line_x + now_line_width/2.0, window.h)
      ],
      now_line_width,
      graphics::BLACK
    ).unwrap();

    graphics::draw(ctx, &line_mesh, graphics::DrawParam::default()).unwrap();

    let arrow_width = 20.0;
    let arrow_height = 10.0;

    let high_mesh = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        nalgebra::Point2::new(0.0, -arrow_height/2.0),
        nalgebra::Point2::new(arrow_width/2.0, arrow_height/2.0),
        nalgebra::Point2::new(-arrow_width/2.0, arrow_height/2.0),
      ],
      graphics::Color::from_rgb(0, 192, 32)
    ).unwrap();

    let low_mesh = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        nalgebra::Point2::new(0.0, arrow_height/2.0),
        nalgebra::Point2::new(-arrow_width/2.0, -arrow_height/2.0),
        nalgebra::Point2::new(arrow_width/2.0, -arrow_height/2.0),
      ],
      graphics::Color::from_rgb(0, 32, 192)
    ).unwrap();

    let note_spacing = window.w/20.0;
    let completion = (self.play_offset.as_secs_f64() / self.module_duration) as f32;
    let completion_offset_x = completion * note_spacing * (self.pattern.len() as f32);

    let instrument = 2;

    for r in 0..self.pattern.len() {
      let x = (r as f32) * note_spacing - completion_offset_x + now_line_x + now_line_width/2.0;
      if x >= (0.0 - arrow_width) && x <= window.w { 
        let cell = &self.pattern[r][instrument];
        if let Some(pitch_info) = cell {
          let mesh = match pitch_info.relative_pitch {
            RelativePitch::High => &high_mesh,
            RelativePitch::Low => &low_mesh,
          };
          let y = window.h - ((pitch_info.pitch as f32) * window.h/128.0);
          graphics::draw(
            ctx,
            mesh,
            graphics::DrawParam::default().dest(nalgebra::Point2::new(x, y))
          ).unwrap();
        }
      }
    }

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
