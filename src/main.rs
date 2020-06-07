extern crate ggez;
extern crate nalgebra;
extern crate rodio;
extern crate midly;

mod assets;
mod counting_source;

use std::{
  convert::TryFrom,
  fs,
  io::BufReader,
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
  dt: Duration,
  play_offset_ms: Arc<AtomicU32>,
  relative_pitch_input: Option<RelativePitchInput>,
  pattern: Vec<PatternNote>,
  sink: Sink,
}

impl State {
  fn new(ctx: &mut Context) -> State {
    let midi_bytes = fs::read("music/weeppiko_musix_-_were_fighting_again.mid").unwrap();
    let midi = Smf::parse(&midi_bytes).unwrap();
    let pattern = get_pattern(&midi);

    let sink = Sink::new(&rodio::default_output_device().unwrap());
    sink.pause();

    let ogg_file = fs::File::open("music/weeppiko_musix_-_were_fighting_again.ogg").unwrap();
    let source = rodio::Decoder::new(BufReader::new(ogg_file)).unwrap();
    let (source, play_offset_ms) = CountingSource::new(source);
    sink.append(source);

    State {
      assets: Assets::new(ctx),
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

      self.relative_pitch_input = None;
    }

    Ok(())
  }

  fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::clear(ctx, graphics::WHITE);

    let window = graphics::screen_coordinates(ctx);

    let now_line_x = 200.0;

    graphics::draw(
      ctx,
      &self.assets.now_line,
      graphics::DrawParam::default().dest(nalgebra::Point2::new(now_line_x, 0.0))
    ).unwrap();

    let spacing_per_second = window.w/4.0;
    let completion_offset_x: f32 = (self.play_offset_ms.load(Ordering::Relaxed) as f32)/1000.0 * spacing_per_second;

    for pattern_note in &self.pattern {
      // FIXME This could certainly be more efficient
      let x = (pattern_note.play_offset_ms as f32)/1000.0 * spacing_per_second - completion_offset_x + now_line_x;
      if x >= (0.0 - self.assets.arrow_width) && x <= window.w { 
        let mesh = match pattern_note.relative_pitch {
          RelativePitch::High => &self.assets.up_arrow,
          RelativePitch::Low => &self.assets.down_arrow,
        };
        let y = window.h - ((pattern_note.pitch as f32 - 64.0) * window.h/32.0 + 300.0);
        graphics::draw(
          ctx,
          mesh,
          graphics::DrawParam::default().dest(nalgebra::Point2::new(x, y))
        ).unwrap();
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
  let c = conf::Conf::new();
  let (ref mut ctx, ref mut event_loop) = ggez::ContextBuilder::new("Upbeat", "David Simon")
    .conf(c)
    .build()
    .unwrap();

  let state = &mut State::new(ctx);
  event::run(ctx, event_loop, state).unwrap();
}
