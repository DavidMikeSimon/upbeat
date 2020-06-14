extern crate ggez;
extern crate itertools;
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
use itertools::Itertools;
use rodio::{Sink, Source};
use midly::{Smf, Format, EventKind, MidiMessage, MetaMessage, Timing};

use assets::Assets;
use counting_source::CountingSource;

const TARGET_TRACK: usize = 12;
const LEAD_IN_MSEC: u32 = 4000; // TODO: Actual duration seems to be about 1/4th this?

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
  match midi.header.format {
    Format::Parallel => {}, // OK
    _ => panic!("MIDI file must be in parallel (simultaneous tracks) format")
  }

  let ticks_per_beat = match midi.header.timing {
    Timing::Metrical(n) => n.as_int(),
    _ => panic!("MIDI timing must be metrical")
  };
  let ticks_per_beat: f64 = ticks_per_beat.into();

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

  midi.tracks[TARGET_TRACK]
    .iter()
    .scan(0.0, |play_offset_ms, &event| {
      *play_offset_ms = *play_offset_ms + event.delta.as_int() as f64 * ms_per_tick;

      match event.kind {
        EventKind::Midi{ message: MidiMessage::NoteOn { key: pitch, .. }, .. } => {
          Some(Some((play_offset_ms.clone(), pitch.as_int())))
        },
        _ => Some(None) // Ignore events other than NoteOn
      }
    })
    .flatten()
    .group_by(|(play_offset_ms, _)| play_offset_ms.clone())
    .into_iter()
    .map(|(play_offset_ms, pitches)| {
      let pitches: Vec<(f64, u8)> = pitches.collect();
      let average_pitch: f64 = pitches.iter().map(|(_, p)| *p as f64).sum::<f64>() / pitches.len() as f64;
      (play_offset_ms, average_pitch.round() as u8)
    })
    .scan((0, RelativePitch::High), |(prior_pitch, prior_relative_pitch), (play_offset_ms, pitch)| {
      let relative_pitch = if pitch == *prior_pitch {
        *prior_relative_pitch
      } else if pitch > *prior_pitch {
        RelativePitch::High
      } else {
        RelativePitch::Low
      };
      let pn = PatternNote {
        play_offset_ms: play_offset_ms as u32,
        pitch: pitch,
        relative_pitch: relative_pitch,
      };
      *prior_relative_pitch = relative_pitch;
      *prior_pitch = pitch;
      Some(pn)
    })
    .collect()
}

struct RelativePitchInput {
  relative_pitch: RelativePitch,
  play_offset_ms: u32
}

struct HeroState {
  position: nalgebra::Point2<f32>,
  hp: u32,
  max_hp: u32
}

struct EnemyState {
  position: nalgebra::Point2<f32>,
  next_move_play_offset_ms: u32,
  ms_between_moves: u32,
  attack_power: u32
}

struct State {
  assets: Assets,
  command_cursor_index: u8,
  dt: Duration,
  play_offset_ms: Arc<AtomicU32>,
  lead_in_offset_ms: Arc<AtomicU32>,
  relative_pitch_input: Option<RelativePitchInput>,
  pattern: Vec<PatternNote>,
  sink: Sink,
  char2_idle_frame: usize,
  heroes: Vec<HeroState>,
  enemies: Vec<EnemyState>,
}

impl State {
  fn new(ctx: &mut Context) -> State {
    let midi_bytes = fs::read("resources/music/goluigi_-_neboke.mid").unwrap();
    let midi = Smf::parse(&midi_bytes).unwrap();
    let pattern = get_pattern(&midi);

    let sink = Sink::new(&rodio::default_output_device().unwrap());
    sink.pause();

    let ogg_file = fs::File::open("resources/music/goluigi_-_neboke.ogg").unwrap();
    let music_source = rodio::Decoder::new(BufReader::new(ogg_file)).unwrap();
    let lead_in_source = rodio::source::Zero::<f32>::new(music_source.channels(), music_source.sample_rate()).take_duration(Duration::from_millis(LEAD_IN_MSEC.into()));

    let (music_source, play_offset_ms) = CountingSource::new(music_source);
    let (lead_in_source, lead_in_offset_ms) = CountingSource::new(lead_in_source);
    sink.append(lead_in_source);
    sink.append(music_source);

    State {
      assets: Assets::new(ctx),
      command_cursor_index: 0,
      dt: Duration::default(),
      play_offset_ms: play_offset_ms,
      lead_in_offset_ms: lead_in_offset_ms,
      relative_pitch_input: None,
      pattern: pattern,
      sink: sink,
      char2_idle_frame: 0,
      heroes: vec![
        HeroState { position: nalgebra::Point2::new(180.0, 35.0), hp: 180, max_hp: 180 },
        HeroState { position: nalgebra::Point2::new(350.0, 110.0), hp: 220, max_hp: 220 },
      ],
      enemies: vec![
        EnemyState {
          position: nalgebra::Point2::new(644.0, 85.0),
          next_move_play_offset_ms: 2000,
          ms_between_moves: 2000,
          attack_power: 50
        },
      ]
    }
  }
}

impl event::EventHandler for State {
  fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::set_window_title(ctx, "Upbeat");
    self.dt = timer::delta(ctx);

    if self.sink.is_paused() { return Ok(()); }

    let play_offset_ms = self.play_offset_ms.load(Ordering::Relaxed);

    for enemy in &mut self.enemies {
      while enemy.next_move_play_offset_ms < play_offset_ms {
        if self.heroes[0].hp > 0 {
          self.heroes[0].hp -= std::cmp::min(enemy.attack_power, self.heroes[0].hp);
        }
        enemy.next_move_play_offset_ms += enemy.ms_between_moves;
      }
    }

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

    for hero in &self.heroes {
      graphics::draw(
        ctx,
        &self.assets.char2_idle[self.char2_idle_frame],
        graphics::DrawParam::default().dest(hero.position)
      ).unwrap();

      graphics::draw(
        ctx,
        &graphics::Text::new((format!("HP: {}/{}", hero.hp, hero.max_hp), self.assets.font, 35.0)),
        graphics::DrawParam::default().dest(hero.position + nalgebra::Vector2::new(20.0, -20.0))
      ).unwrap();
    }

    for enemy in &self.enemies {
      graphics::draw(
        ctx,
        &self.assets.monster,
        graphics::DrawParam::default().dest(enemy.position)
      ).unwrap();
    }

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

    // FIXME Shouldn't have to divide LEAD_IN_MSEC by 4...
    let completion_offset_x: f32 = (self.play_offset_ms.load(Ordering::Relaxed) as f32 - (LEAD_IN_MSEC/4 - self.lead_in_offset_ms.load(Ordering::Relaxed)) as f32)/1000.0 * spacing_per_second;

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

    if self.sink.is_paused() {
      let text = graphics::Text::new(("Press Enter", self.assets.font, 75.0));
      let x = (window.w - text.width(ctx) as f32)/2.0;
      graphics::draw(
        ctx,
        &text,
        graphics::DrawParam::default().dest(nalgebra::Point2::new(x, 250.0))
      ).unwrap();
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
        keyboard::KeyCode::Return => self.sink.play(),
        _ => {}
      }
    } else {
      // TODO: Is the play_offset here slightly off because of time elapsed since last update()?
      match keycode {
        keyboard::KeyCode::Escape => event::quit(ctx),
        keyboard::KeyCode::Return => self.sink.pause(),
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

  let conf = conf::Conf::new()
    .window_mode(conf::WindowMode::default().dimensions(1280.0, 720.0));

  let (ref mut ctx, ref mut event_loop) = ggez::ContextBuilder::new("Upbeat", "David Simon")
    .conf(conf)
    .add_resource_path(resource_dir)
    .build()
    .unwrap();

  let state = &mut State::new(ctx);
  event::run(ctx, event_loop, state).unwrap();
}
