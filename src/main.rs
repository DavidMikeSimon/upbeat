extern crate ggez;
extern crate nalgebra;
extern crate rodio;
extern crate midly;

use std::{
  convert::TryInto,
  fs,
  io::BufReader,
  time::Duration
};

use ggez::{conf, event, graphics, timer, input::keyboard, Context, GameResult};
use rodio::{Sink};
use midly::{Smf, Format, EventKind, MidiMessage};

const TARGET_TRACK: usize = 10;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RelativePitch {
  High,
  Low
}

struct PitchInfo {
  pitch: u8,
  relative_pitch: RelativePitch,
}

fn get_duration_seconds(midi: &Smf) -> f64 {
  120.0
}

fn get_pattern(midi: &Smf) -> Vec<Vec<Option<PitchInfo>>> {
  let mut r_pattern = Vec::new();

  match midi.header.format {
    Format::Parallel => {}, // OK
    _ => panic!("MIDI file must be in parallel (simultaneous tracks) format")
  }

  println!("midi timing {:?}", midi.header.timing);

  for track in &midi.tracks {
    let mut prior_pitch = 0;
    let mut prior_relative_pitch = RelativePitch::High;
    let mut track_pattern = Vec::new();

    for event in track {
      match event.kind {
        EventKind::Midi{ message: msg, .. } => {
          match msg {
            MidiMessage::NoteOn { key: pitch, .. } => {
              let pitch = pitch.as_int();
              let relative_pitch = if pitch == prior_pitch {
                prior_relative_pitch
              } else if pitch > prior_pitch {
                RelativePitch::High
              } else {
                RelativePitch::Low
              };
              track_pattern.push(Some(PitchInfo {
                pitch: pitch,
                relative_pitch: relative_pitch,
              }));
              prior_relative_pitch = relative_pitch;
              prior_pitch = pitch;
            },
            _ => {} // Ignore
          }
        },
        _ => {} // Ignore
      }
    }

    r_pattern.push(track_pattern);
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
  song_duration: f64,
  pattern: Vec<Vec<Option<PitchInfo>>>,
  sink: Sink,
}

impl State {
  fn new() -> State {

    let midi_bytes = fs::read("music/weeppiko_musix_-_were_fighting_again.mid").unwrap();
    let midi = Smf::parse(&midi_bytes).unwrap();
    let song_duration = get_duration_seconds(&midi);
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
      song_duration: song_duration,
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
      let beats_per_second = (self.pattern[TARGET_TRACK].len() as f64)/self.song_duration;
      let input_note_index: isize = (input.play_offset.as_secs_f64() * beats_per_second.round()) as isize;
      let mut nearest_note_index: Option<usize> = None;
      let mut nearest_note_offset_ms: f64 = 0.0;
      for index_offset in -1..1 {
        let idx: isize = input_note_index + index_offset;
        if idx < 0 { continue; }
        let idx: usize = idx.try_into().unwrap();
        if idx >= self.pattern[TARGET_TRACK].len() { continue; }
        if self.pattern[TARGET_TRACK][idx].is_none() { continue; }

        let this_offset_ms = (input.play_offset.as_secs_f64() - (idx as f64)/beats_per_second) * 1000.0;
        match nearest_note_index {
          None => {
            nearest_note_index = Some(idx);
            nearest_note_offset_ms = this_offset_ms;
          },
          Some(_) => {
            if this_offset_ms.abs() < nearest_note_offset_ms.abs() {
              nearest_note_index = Some(idx);
              nearest_note_offset_ms = this_offset_ms;
            }
          }
        }
      }

      if let Some(idx) = nearest_note_index {
        let relative_pitch_ok = input.relative_pitch == self.pattern[TARGET_TRACK][idx].as_ref().unwrap().relative_pitch;
        println!("{:03} MATCH {:5}: {:+5.2}msec", idx, relative_pitch_ok, nearest_note_offset_ms);
      } else {
        println!("NO MATCH");
      }

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

    let note_spacing = window.w/20.0;
    let completion = (self.play_offset.as_secs_f64() / self.song_duration) as f32;
    let completion_offset_x = completion * note_spacing * (self.pattern[TARGET_TRACK].len() as f32);

    for r in 0..self.pattern[TARGET_TRACK].len() {
      let x = (r as f32) * note_spacing - completion_offset_x + now_line_x + now_line_width/2.0;
      if x >= (0.0 - arrow_width) && x <= window.w { 
        let cell = &self.pattern[TARGET_TRACK][r];
        if let Some(pitch_info) = cell {
          let mesh = match pitch_info.relative_pitch {
            RelativePitch::High => &high_mesh,
            RelativePitch::Low => &low_mesh,
          };
          let y = window.h - ((pitch_info.pitch as f32 - 64.0) * window.h/32.0 + 300.0);
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
