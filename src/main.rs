extern crate ggez;
extern crate nalgebra;
extern crate rodio;
extern crate midly;

mod assets;
mod counting_source;

use std::{
  convert::TryFrom,
  env,
  fs,
  io::BufReader,
  path,
  time::Duration,
  sync::{Arc, atomic::{AtomicU32, Ordering}},
};

use ggez::{conf, event, graphics, timer, input::keyboard, Context, GameResult};
use rodio::{Sink};
use midly::{Smf, Format, EventKind, MidiMessage, MetaMessage, Timing};

use assets::Assets;
use counting_source::CountingSource;

const TARGET_TRACK: usize = 10;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RelativePitch {
  High,
  Low
}

struct PatternNote {
  play_offset_ms: u32,
  pitch: u8,
  relative_pitch: RelativePitch,
}

fn get_pattern(midi: &Smf) -> Vec<PatternNote> {
  let mut r_pattern = Vec::new();

  match midi.header.format {
    Format::Parallel => {}, // OK
    _ => panic!("MIDI file must be in parallel (simultaneous tracks) format")
  }

  let ticks_per_beat = match midi.header.timing {
    Timing::Metrical(n) => n.as_int(),
    _ => panic!("MIDI timing must be metrical")
  };
  let ticks_per_beat: f64 = ticks_per_beat.into();

  let mut prior_pitch = 0;
  let mut prior_relative_pitch = RelativePitch::High;
  let mut play_offset_ms = 0.0;

  let mut microseconds_per_beat = 0;
  for event in &midi.tracks[0] { // Track 0 is the global timing track
    if let EventKind::Meta(MetaMessage::Tempo(mspb)) = event.kind {
      microseconds_per_beat = mspb.as_int();
    }
  }
  if microseconds_per_beat == 0 {
    panic!("MIDI track 0 must include tempo information");
  }
  let ms_per_beat: f64 = (microseconds_per_beat as f64)/1000.0;
  let ms_per_tick = ms_per_beat/ticks_per_beat;

  for event in &midi.tracks[TARGET_TRACK] {
    play_offset_ms += (event.delta.as_int() as f64) * ms_per_tick;

    match event.kind {
      EventKind::Midi{ message: MidiMessage::NoteOn { key: pitch, .. }, .. } => {
        let pitch = pitch.as_int();
        let relative_pitch = if pitch == prior_pitch {
          prior_relative_pitch
        } else if pitch > prior_pitch {
          RelativePitch::High
        } else {
          RelativePitch::Low
        };
        r_pattern.push(PatternNote {
          play_offset_ms: play_offset_ms as u32,
          pitch: pitch,
          relative_pitch: relative_pitch,
        });
        prior_relative_pitch = relative_pitch;
        prior_pitch = pitch;
      },
      _ => {} // Ignore
    }
  }

  r_pattern
}

struct RelativePitchInput {
  relative_pitch: RelativePitch,
  play_offset_ms: u32
}

struct State {
  assets: Assets,
  command_cursor_index: u8,
  dt: Duration,
  play_offset_ms: Arc<AtomicU32>,
  relative_pitch_input: Option<RelativePitchInput>,
  pattern: Vec<PatternNote>,
  sink: Sink,
}

impl State {
  fn new(ctx: &mut Context) -> State {
    let midi_bytes = fs::read("resources/music/weeppiko_musix_-_were_fighting_again.mid").unwrap();
    let midi = Smf::parse(&midi_bytes).unwrap();
    let pattern = get_pattern(&midi);

    let sink = Sink::new(&rodio::default_output_device().unwrap());
    sink.pause();

    let ogg_file = fs::File::open("resources/music/weeppiko_musix_-_were_fighting_again.ogg").unwrap();
    let source = rodio::Decoder::new(BufReader::new(ogg_file)).unwrap();
    let (source, play_offset_ms) = CountingSource::new(source);
    sink.append(source);

    State {
      assets: Assets::new(ctx),
      command_cursor_index: 0,
      dt: Duration::default(),
      play_offset_ms: play_offset_ms,
      relative_pitch_input: None,
      pattern: pattern,
      sink: sink
    }
  }
}

impl event::EventHandler for State {
  fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::set_window_title(ctx, "Upbeat");
    self.dt = timer::delta(ctx);

    if self.sink.is_paused() { return Ok(()); }

    if let Some(input) = &self.relative_pitch_input {
      let nearest_pattern_note = self.pattern
        .iter()
        .min_by_key(|pn| ((pn.play_offset_ms as i32) - (input.play_offset_ms as i32)).abs())
        .unwrap();

      let nearest_note_offset_ms: i32 = i32::try_from(input.play_offset_ms).unwrap() - i32::try_from(nearest_pattern_note.play_offset_ms).unwrap();
      let relative_pitch_ok = input.relative_pitch == nearest_pattern_note.relative_pitch;
      println!("MATCH {:5}: {:+3}msec", relative_pitch_ok, nearest_note_offset_ms);

      self.command_cursor_index = match (input.relative_pitch, self.command_cursor_index) {
        (RelativePitch::High, 0) => 3,
        (RelativePitch::High, _) => self.command_cursor_index - 1,
        (RelativePitch::Low, 3) => 0,
        (RelativePitch::Low, _) => self.command_cursor_index + 1,
      };

      self.relative_pitch_input = None;
    }

    Ok(())
  }

  fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::clear(ctx, graphics::WHITE);

    let window = graphics::screen_coordinates(ctx);

    graphics::draw(
      ctx,
      &self.assets.bg,
      graphics::DrawParam::default()
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.char1,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(50.0, 100.0)).scale(nalgebra::Vector2::new(0.5, 0.5))
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.char2,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(80.0, 200.0)).scale(nalgebra::Vector2::new(0.5, 0.5))
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.monster,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(600.0, 200.0)).scale(nalgebra::Vector2::new(0.5, 0.5))
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.music_bar,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(self.assets.command_window_width, window.h - self.assets.music_bar_height))
    ).unwrap();

    let now_line_x = self.assets.command_window_width + self.assets.now_line_x_offset;

    graphics::draw(
      ctx,
      &self.assets.now_line,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(now_line_x, window.h - self.assets.music_bar_height))
    ).unwrap();

    let spacing_per_second = window.w/5.0;
    let music_bar_min_pitch = 55;
    let music_bar_max_pitch = 75;

    let completion_offset_x: f32 = (self.play_offset_ms.load(Ordering::Relaxed) as f32)/1000.0 * spacing_per_second;

    // FIXME This could certainly be more efficient by remembering where it left off last time
    for pattern_note in &self.pattern {
      let x = (pattern_note.play_offset_ms as f32)/1000.0 * spacing_per_second - completion_offset_x + now_line_x;
      if x >= (0.0 - self.assets.arrow_width) && x <= window.w { 
        let mesh = match pattern_note.relative_pitch {
          RelativePitch::High => &self.assets.up_arrow,
          RelativePitch::Low => &self.assets.down_arrow,
        };
        let pitch_amt = ((pattern_note.pitch - music_bar_min_pitch) as f32)/((music_bar_max_pitch - music_bar_min_pitch) as f32);
        let y = window.h - self.assets.music_bar_height*pitch_amt;
        graphics::draw(
          ctx,
          mesh,
          graphics::DrawParam::default().dest(nalgebra::Point2::new(x, y))
        ).unwrap();
      }
    }

    graphics::draw(
      ctx,
      &self.assets.command_window,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(0.0, window.h - self.assets.music_bar_height))
    ).unwrap();

    let left_margin = 50.0;
    let top_margin = 5.0;
    let font_size = 55.0;
    let line_spacing = 45.0;

    graphics::draw(
      ctx,
      &graphics::Text::new(("Strike", self.assets.font, font_size)),
      graphics::DrawParam::default().dest(nalgebra::Point2::new(left_margin, window.h - self.assets.music_bar_height + top_margin))
    ).unwrap();

    graphics::draw(
      ctx,
      &graphics::Text::new(("Hold", self.assets.font, font_size)),
      graphics::DrawParam::default().dest(nalgebra::Point2::new(left_margin, window.h - self.assets.music_bar_height + top_margin + line_spacing))
    ).unwrap();

    graphics::draw(
      ctx,
      &graphics::Text::new(("Magic", self.assets.font, font_size)),
      graphics::DrawParam::default().dest(nalgebra::Point2::new(left_margin, window.h - self.assets.music_bar_height + top_margin + line_spacing*2.0))
    ).unwrap();

    graphics::draw(
      ctx,
      &graphics::Text::new(("Items", self.assets.font, font_size)),
      graphics::DrawParam::default().dest(nalgebra::Point2::new(left_margin, window.h - self.assets.music_bar_height + top_margin + line_spacing*3.0))
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.command_cursor,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(left_margin/2.0, window.h - self.assets.music_bar_height + top_margin + font_size*0.5 + line_spacing*(self.command_cursor_index as f32)))
    ).unwrap();

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

    if self.sink.is_paused() {
      match keycode {
        keyboard::KeyCode::Escape => event::quit(ctx),
        keyboard::KeyCode::Space => self.sink.play(),
        _ => {}
      }
    } else {
      // TODO: Is the play_offset here slightly off because of time elapsed since last update()?
      match keycode {
        keyboard::KeyCode::Escape => event::quit(ctx),
        keyboard::KeyCode::Space => self.sink.pause(),
        keyboard::KeyCode::Up => self.relative_pitch_input = Some(RelativePitchInput {
          relative_pitch: RelativePitch::High,
          play_offset_ms: self.play_offset_ms.load(Ordering::Relaxed),
        }),
        keyboard::KeyCode::Down => self.relative_pitch_input = Some(RelativePitchInput {
          relative_pitch: RelativePitch::Low,
          play_offset_ms: self.play_offset_ms.load(Ordering::Relaxed),
        }),
        _ => {}
      }
    }
  }
}

fn main() {
  let mut resource_dir = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
    path::PathBuf::from(manifest_dir)
  } else {
    path::PathBuf::from(".")
  };
  resource_dir.push("resources");

  let c = conf::Conf::new();
  let (ref mut ctx, ref mut event_loop) = ggez::ContextBuilder::new("Upbeat", "David Simon")
    .conf(c)
    .add_resource_path(resource_dir)
    .build()
    .unwrap();

  let state = &mut State::new(ctx);
  event::run(ctx, event_loop, state).unwrap();
}
