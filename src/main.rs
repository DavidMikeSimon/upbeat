extern crate ggez;
extern crate nalgebra;
extern crate rodio;
extern crate midly;

use std::{
  fs,
  io::BufReader,
  time::Duration
};

use ggez::{conf, event, graphics, timer, input::keyboard, Context, GameResult};
use rodio::{Sink};
use midly::{Smf, Format, EventKind, MidiMessage, MetaMessage, Timing};

const TARGET_TRACK: usize = 10;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RelativePitch {
  High,
  Low
}

struct PatternNote {
  play_offset: f64,
  pitch: u8,
  relative_pitch: RelativePitch,
}

fn get_pattern(midi: &Smf) -> Vec<PatternNote> {
  let mut r_pattern = Vec::new();

  match midi.header.format {
    Format::Parallel => {}, // OK
    _ => panic!("MIDI file must be in parallel (simultaneous tracks) format")
  }

  let ticks_per_beat;
  if let Timing::Metrical(tpb) = midi.header.timing {
    ticks_per_beat = tpb.as_int();
  } else {
    panic!("MIDI timing must be metrical");
  }
  let ticks_per_beat = ticks_per_beat as f64;

  let mut prior_pitch = 0;
  let mut prior_relative_pitch = RelativePitch::High;
  let mut play_offset = 0.0;

  let mut ms_per_beat = 0;
  for event in &midi.tracks[0] { // Track 0 is the global timing track
    if let EventKind::Meta(MetaMessage::Tempo(mspb)) = event.kind {
      ms_per_beat = mspb.as_int();
    }
  }
  if ms_per_beat == 0 {
    panic!("MIDI track 0 must include tempo information");
  }
  let ms_per_beat = ms_per_beat as f64;

  for event in &midi.tracks[TARGET_TRACK] {
    play_offset += ((event.delta.as_int() as f64) * ms_per_beat)/(ticks_per_beat*1000.0*1000.0);

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
          play_offset: play_offset,
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
  play_offset: Duration
}

struct State {
  dt: Duration,
  play_offset: Duration,
  relative_pitch_input: Option<RelativePitchInput>,
  pattern: Vec<PatternNote>,
  sink: Sink,
}

impl State {
  fn new() -> State {

    let midi_bytes = fs::read("music/weeppiko_musix_-_were_fighting_again.mid").unwrap();
    let midi = Smf::parse(&midi_bytes).unwrap();
    let pattern = get_pattern(&midi);

    let sink = Sink::new(&rodio::default_output_device().unwrap());
    sink.pause();

    let ogg_file = fs::File::open("music/weeppiko_musix_-_were_fighting_again.ogg").unwrap();
    let source = rodio::Decoder::new(BufReader::new(ogg_file)).unwrap();
    sink.append(source);

    State {
      dt: Duration::default(),
      play_offset: Duration::default(),
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

    self.play_offset += self.dt;

    if let Some(input) = &self.relative_pitch_input {
      let nearest_pattern_note = self.pattern
        .iter()
        .min_by_key(|pn| {
          let diff_sec = (pn.play_offset - input.play_offset.as_secs_f64()).abs();
          (diff_sec * 1_000_000.0) as u32
        })
        .unwrap();

      let nearest_note_offset = input.play_offset.as_secs_f64() - nearest_pattern_note.play_offset;
      let relative_pitch_ok = input.relative_pitch == nearest_pattern_note.relative_pitch;
      println!("MATCH {:5}: {:+5.2}msec", relative_pitch_ok, nearest_note_offset * 1000.0);

      self.relative_pitch_input = None;
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

    let spacing_per_second = window.w/4.0;
    let completion_offset_x: f32 = (self.play_offset.as_secs_f64() as f32) * spacing_per_second;

    for pattern_note in &self.pattern {
      // FIXME This could certainly be more efficient
      let x = (pattern_note.play_offset as f32) * spacing_per_second - completion_offset_x + now_line_x + now_line_width/2.0;
      if x >= (0.0 - arrow_width) && x <= window.w { 
        let mesh = match pattern_note.relative_pitch {
          RelativePitch::High => &high_mesh,
          RelativePitch::Low => &low_mesh,
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
          play_offset: self.play_offset,
        }),
        keyboard::KeyCode::Down => self.relative_pitch_input = Some(RelativePitchInput {
          relative_pitch: RelativePitch::Low,
          play_offset: self.play_offset,
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

  let state = &mut State::new();
  event::run(ctx, event_loop, state).unwrap();
}
