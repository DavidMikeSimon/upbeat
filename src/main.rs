#[macro_use] extern crate maplit;

extern crate ggez;
extern crate itertools;
extern crate midly;
extern crate nalgebra;
extern crate rodio;

mod anim;
mod assets;
mod counting_source;

use std::{
  collections::HashMap,
  convert::TryFrom,
  env,
  fs,
  io::BufReader,
  path,
  time::Duration,
  sync::{Arc, atomic::{AtomicU32, Ordering}},
};

use ggez::{conf, event, graphics, timer, input::keyboard::{KeyCode, KeyMods}, Context, GameResult};
use itertools::Itertools;
use rodio::{Sink, Source};
use midly::{Smf, Format, EventKind, MidiMessage, MetaMessage, Timing};
use nalgebra::{Point2, Vector2};

use assets::Assets;
use counting_source::CountingSource;

const MIDI_PATH: &str = "resources/music/weeppiko_musix_-_were_fighting_again.mid";
const OGG_PATH: &str = "resources/music/weeppiko_musix_-_were_fighting_again.ogg";
const TARGET_TRACKS: [usize; 2] = [10, 28];
const LEAD_IN_MSEC: u32 = 1000;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RelativePitch {
  High,
  Low
}

#[derive(Copy, Clone, Debug)]
struct MidiTiming {
  ms_per_beat: f32,
  ms_per_tick: f32,
  beats_per_measure: f32,
}

struct PatternNote {
  time: u32,
  pitch: u8,
  relative_pitch: RelativePitch,
}

fn get_timing(midi: &Smf) -> MidiTiming {
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
  let mut beats_per_measure: Option<u8> = None;
  for event in &midi.tracks[0] { // Track 0 is the global timing track
    match event.kind {
      EventKind::Meta(MetaMessage::Tempo(mspb)) => {
        microseconds_per_beat = mspb.as_int();
      },
      EventKind::Meta(MetaMessage::TimeSignature(numerator, _, _, _)) => {
        beats_per_measure = match beats_per_measure {
          None => Some(numerator),
          Some(n) if n == numerator => Some(numerator),
          _ => panic!("Multiple conflicting time signatures found"),
        }
      },
      _ => {}
    }
  }
  let beats_per_measure = beats_per_measure.unwrap_or_else(|| panic!("No time signature found"));
  if microseconds_per_beat == 0 {
    panic!("MIDI track 0 must include tempo information");
  }
  let ms_per_beat: f64 = (microseconds_per_beat as f64)/1000.0;
  let ms_per_tick = ms_per_beat/ticks_per_beat;

  MidiTiming {
    ms_per_beat: ms_per_beat as f32,
    ms_per_tick: ms_per_tick as f32,
    beats_per_measure: beats_per_measure as f32
  }
}

fn get_pattern(midi: &Smf, timing: &MidiTiming) -> Vec<PatternNote> {
  TARGET_TRACKS
    .iter()
    .flat_map(|&track_idx| {
      midi.tracks[track_idx].iter().scan(0.0, |time, &event| {
        *time = *time + event.delta.as_int() as f32 * timing.ms_per_tick;

        match event.kind {
          EventKind::Midi{ message: MidiMessage::NoteOn { key: pitch, .. }, .. } => {
            Some(Some((time.clone(), pitch.as_int())))
          },
          _ => Some(None) // Ignore events other than NoteOn
        }
      })
    })
    .flatten()
    .group_by(|(time, _)| time.clone())
    .into_iter()
    .map(|(time, pitches)| {
      let pitches: Vec<(f32, u8)> = pitches.collect();
      let average_pitch: f32 = pitches.iter().map(|(_, p)| *p as f32).sum::<f32>() / pitches.len() as f32;
      (time, average_pitch.round() as u8)
    })
    .scan((0, RelativePitch::High), |(prior_pitch, prior_relative_pitch), (time, pitch)| {
      let relative_pitch = if pitch == *prior_pitch {
        *prior_relative_pitch
      } else if pitch > *prior_pitch {
        RelativePitch::High
      } else {
        RelativePitch::Low
      };
      let pn = PatternNote {
        time: time as u32,
        pitch: pitch,
        relative_pitch: relative_pitch,
      };
      *prior_relative_pitch = relative_pitch;
      *prior_pitch = pitch;
      Some(pn)
    })
    .collect()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NavDirection {
  Up,
  Right,
  Down,
  Left,
}

struct RelativePitchInput {
  direction: NavDirection,
  relative_pitch: RelativePitch,
  time: u32,
}

struct HeroState {
  character: usize,
  position: Point2<f32>,
  attack_power: u32,
  hp: u32,
  max_hp: u32,
}

struct EnemyState {
  position: Point2<f32>,
  attack_power: u32,
  hp: u32,
  max_hp: u32,
}

enum ActionSource {
  Hero { idx: usize },
  Enemy { idx: usize },
}

enum ActionTarget {
  Hero { idx: usize },
  Enemy { idx: usize },
}

enum CombatAction {
  Attack { src: ActionSource, tgt: ActionTarget }
}

struct BgAnim {
  animation: anim::Animation,
  position: Point2<f32>,
  scale: Vector2<f32>
}

struct State {
  assets: Assets,
  bg_anims: Vec<BgAnim>,
  dt: Duration,
  time: Arc<AtomicU32>,
  lead_in_offset_ms: Arc<AtomicU32>,
  relative_pitch_input: Option<RelativePitchInput>,
  timing: MidiTiming,
  pattern: Vec<PatternNote>,
  sink: Sink,
  heroes: Vec<HeroState>,
  enemies: Vec<EnemyState>,
  actions: HashMap<usize, CombatAction>,
  command_window_hero: usize,
  last_measure_action_processed: Option<usize>,
}

impl State {
  fn new(ctx: &mut Context) -> State {
    let midi_bytes = fs::read(MIDI_PATH).unwrap();
    let midi = Smf::parse(&midi_bytes).unwrap();
    let timing = get_timing(&midi);
    let pattern = get_pattern(&midi, &timing);

    let sink = Sink::new(&rodio::default_output_device().unwrap());
    // sink.set_volume(0.0);
    sink.pause();

    let ogg_file = fs::File::open(OGG_PATH).unwrap();
    let music_source = rodio::Decoder::new(BufReader::new(ogg_file)).unwrap();
    // FIXME: Shouldn't have to multiply LEAD_IN_MSEC by 4, is this a rodio bug?
    let lead_in_source = rodio::source::Zero::<f32>::new(music_source.channels(), music_source.sample_rate()).take_duration(Duration::from_millis((LEAD_IN_MSEC*4).into()));

    let (music_source, time) = CountingSource::new(music_source);
    let (lead_in_source, lead_in_offset_ms) = CountingSource::new(lead_in_source);
    sink.append(lead_in_source);
    sink.append(music_source);

    let assets = Assets::new(ctx);

    let bg_anims = vec!(
      BgAnim {
        animation: anim::Animation::new(assets.sky_anim.clone()),
        position: Point2::<f32>::new(0.0, 0.0),
        scale: Vector2::<f32>::new(1.0, 1.0),
      },
      BgAnim {
        animation: anim::Animation::new(assets.grass_anim.clone()),
        position: Point2::<f32>::new(0.0, 5.0),
        scale: Vector2::<f32>::new(1.0, 1.0),
      },
      BgAnim {
        animation: anim::Animation::new(assets.rocks_anim.clone()),
        position: Point2::<f32>::new(79.0, 342.0),
        scale: Vector2::<f32>::new(1.0, 1.0),
      },
      BgAnim {
        animation: anim::Animation::new(assets.dirt_anim.clone()),
        position: Point2::<f32>::new(0.0, 355.0),
        scale: Vector2::<f32>::new(1.0, 1.0),
      },
      BgAnim {
        animation: anim::Animation::new(assets.left_bush_anim.clone()),
        position: Point2::<f32>::new(0.0, 374.46),
        scale: Vector2::<f32>::new(1.0, 1.0),
      }
    );

    State {
      assets: assets,
      bg_anims: bg_anims,
      dt: Duration::default(),
      time: time,
      lead_in_offset_ms: lead_in_offset_ms,
      relative_pitch_input: None,
      timing: timing,
      pattern: pattern,
      sink: sink,
      heroes: vec![
        HeroState {
          character: 0,
          position: Point2::new(260.0, 113.0),
          attack_power: 50,
          hp: 180,
          max_hp: 180
        }
      ],
      enemies: vec![
        EnemyState {
          position: Point2::new(644.0, 120.0),
          attack_power: 80,
          hp: 400,
          max_hp: 400,
        },
      ],
      actions: hashmap![
        2 => CombatAction::Attack{src: ActionSource::Enemy{idx: 0}, tgt: ActionTarget::Hero{idx: 0}},
        3 => CombatAction::Attack{src: ActionSource::Hero{idx: 0}, tgt: ActionTarget::Enemy{idx: 0}},
        4 => CombatAction::Attack{src: ActionSource::Enemy{idx: 0}, tgt: ActionTarget::Hero{idx: 0}},
        5 => CombatAction::Attack{src: ActionSource::Hero{idx: 0}, tgt: ActionTarget::Enemy{idx: 0}},
        6 => CombatAction::Attack{src: ActionSource::Enemy{idx: 0}, tgt: ActionTarget::Hero{idx: 0}},
        7 => CombatAction::Attack{src: ActionSource::Hero{idx: 0}, tgt: ActionTarget::Enemy{idx: 0}},
        8 => CombatAction::Attack{src: ActionSource::Enemy{idx: 0}, tgt: ActionTarget::Hero{idx: 0}},
      ],
      command_window_hero: 0,
      last_measure_action_processed: None
    }
  }

  fn draw_command_window(&self, ctx: &mut Context, hero: &HeroState) {
    let center_point = Point2::new(hero.position.x + 60.0, hero.position.y + 70.0);

    graphics::draw(
      ctx,
      &self.assets.cursor,
      graphics::DrawParam::default().dest(center_point)
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.button,
      graphics::DrawParam::default().dest(center_point + Vector2::new(self.assets.button_width + self.assets.button_margin, 0.0))
    ).unwrap();

    graphics::draw(
      ctx,
      &graphics::Text::new(("Stk", self.assets.font, 40.0)),
      graphics::DrawParam::default().dest(center_point + Vector2::new(self.assets.button_width + self.assets.button_margin + 10.0, 10.0))
    ).unwrap();

    graphics::draw(
      ctx,
      &self.assets.button,
      graphics::DrawParam::default().dest(center_point - Vector2::new(self.assets.button_width + self.assets.button_margin, 0.0))
    ).unwrap();

    graphics::draw(
      ctx,
      &graphics::Text::new(("Rst", self.assets.font, 40.0)),
      graphics::DrawParam::default().dest(center_point + Vector2::new(-self.assets.button_width, 10.0))
    ).unwrap();

  }
}

impl event::EventHandler for State {
  fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::set_window_title(ctx, "Upbeat");
    self.dt = timer::delta(ctx);

    if self.sink.is_paused() { return Ok(()); }

    let time = self.time.load(Ordering::Relaxed);

    let current_measure_idx = (time as usize)/((self.timing.beats_per_measure * self.timing.ms_per_beat).trunc() as usize);
    let is_next_measure = match self.last_measure_action_processed {
      None => true,
      Some(last_measure_processed_idx) => current_measure_idx > last_measure_processed_idx
    };
    if is_next_measure {
      match self.actions.get(&current_measure_idx) {
        None => {},
        Some(action) => match action {
          CombatAction::Attack{ src, tgt } => {
            let attack_power = match *src {
              ActionSource::Hero{ idx } => self.heroes[idx].attack_power,
              ActionSource::Enemy{ idx } => self.enemies[idx].attack_power
            };

            match *tgt {
              ActionTarget::Hero{ idx } => {
                if self.heroes[idx].hp > 0 {
                  self.heroes[idx].hp -= std::cmp::min(attack_power, self.heroes[0].hp);
                }
              },
              ActionTarget::Enemy { idx }=> {
                if self.enemies[idx].hp > 0 {
                  self.enemies[idx].hp -= std::cmp::min(attack_power, self.enemies[0].hp);
                }
              },
            }
          }
        }
      }

      self.last_measure_action_processed = Some(current_measure_idx)
    }

    if let Some(input) = &self.relative_pitch_input {
      let nearest_pattern_note = self.pattern
        .iter()
        .min_by_key(|pn| ((pn.time as i32) - (input.time as i32)).abs())
        .unwrap();

      let nearest_note_offset_ms: i32 = i32::try_from(input.time).unwrap() - i32::try_from(nearest_pattern_note.time).unwrap();
      let relative_pitch_ok = input.relative_pitch == nearest_pattern_note.relative_pitch;
      println!("MATCH {:5}: {:+4}msec (T:{:+7})", relative_pitch_ok, nearest_note_offset_ms, nearest_pattern_note.time);

      self.relative_pitch_input = None;
    }

    Ok(())
  }

  fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
    graphics::clear(ctx, graphics::WHITE);

    let window = graphics::screen_coordinates(ctx);
    let time = self.time.load(Ordering::Relaxed);

    for bg_anim in self.bg_anims.iter() {
      graphics::draw(
        ctx,
        bg_anim.animation.get_frame(time, self.timing.ms_per_beat),
        graphics::DrawParam::default().dest(bg_anim.position).scale(bg_anim.scale)
      ).unwrap();
    }

    for (i, hero) in self.heroes.iter().enumerate() {
      graphics::draw(
        ctx,
        match hero.character {
          0 => &self.assets.char1,
          1 => &self.assets.char2,
          _ => panic!("Unknown hero character idx")
        },
        graphics::DrawParam::default().dest(hero.position)
      ).unwrap();

      if self.command_window_hero == i {
        self.draw_command_window(ctx, &hero);
      }

      graphics::draw(
        ctx,
        &graphics::Text::new((format!("HP: {}/{}", hero.hp, hero.max_hp), self.assets.font, 30.0)),
        graphics::DrawParam::default().dest(hero.position + Vector2::new(50.0, 380.0)).color(graphics::BLACK)
      ).unwrap();
    }

    for enemy in &self.enemies {
      graphics::draw(
        ctx,
        &self.assets.monster,
        graphics::DrawParam::default().dest(enemy.position)
      ).unwrap();

      graphics::draw(
        ctx,
        &graphics::Text::new((format!("HP: {}/{}", enemy.hp, enemy.max_hp), self.assets.font, 30.0)),
        graphics::DrawParam::default().dest(enemy.position + Vector2::new(200.0, 300.0)).color(graphics::BLACK)
      ).unwrap();
    }

    graphics::draw(
      ctx,
      &self.assets.music_bar,
      graphics::DrawParam::default().dest(Point2::new(0.0, window.h - self.assets.music_bar_height))
    ).unwrap();

    let now_line_x = self.assets.now_line_x_offset;

    graphics::draw(
      ctx,
      &self.assets.now_line,
      graphics::DrawParam::default().dest(Point2::new(now_line_x, window.h - self.assets.music_bar_height))
    ).unwrap();

    let spacing_per_second = window.w/5.0;
    let music_bar_min_pitch = 45;
    let music_bar_max_pitch = 95;

    let completion_offset_x: f32 = (time as f32 - (LEAD_IN_MSEC - self.lead_in_offset_ms.load(Ordering::Relaxed)) as f32)/1000.0 * spacing_per_second;

    // FIXME: This could _definitely_ be done more efficiently and correctly
    for measure_idx in 0..100 {
      let x = (measure_idx as f32) * self.timing.beats_per_measure * (self.timing.ms_per_beat/1000.0) * spacing_per_second - completion_offset_x + now_line_x;
      if x >= 0.0 && x <= window.w {
        graphics::draw(
          ctx,
          &self.assets.measure_line,
          graphics::DrawParam::default().dest(Point2::new(x, window.h - self.assets.music_bar_height))
        ).unwrap();

        if let Some(action) = self.actions.get(&measure_idx) {
          let action_indicator_color = match action {
            CombatAction::Attack { src: ActionSource::Hero { .. }, .. } => graphics::Color::from_rgba(0, 0, 255, 128),
            CombatAction::Attack { src: ActionSource::Enemy { .. }, .. } => graphics::Color::from_rgba(255, 0, 0, 128),
          };

          graphics::draw(
            ctx,
            &self.assets.measure_action_indicator,
            graphics::DrawParam::default()
              .dest(Point2::new(x, window.h - (self.assets.music_bar_height + 20.0)))
              .color(action_indicator_color)
          ).unwrap();

          let action_time = (measure_idx as u32) * (self.timing.beats_per_measure * self.timing.ms_per_beat) as u32;

          match action {
            CombatAction::Attack { src, tgt } => {
              let src_pos = match *src {
                ActionSource::Hero{ idx } => self.heroes[idx].position + Vector2::new(200.0, 180.0),
                ActionSource::Enemy{ idx } => self.enemies[idx].position + Vector2::new(220.0, 165.0),
              };

              let tgt_pos = match *tgt {
                ActionTarget::Hero{ idx } => self.heroes[idx].position + Vector2::new(90.0, 180.0),
                ActionTarget::Enemy{ idx } => self.enemies[idx].position + Vector2::new(180.0, 145.0),
              };

              let color = match src {
                ActionSource::Hero{ .. } => graphics::Color::from_rgba(0, 0, 255, 192),
                ActionSource::Enemy{ .. } => graphics::Color::from_rgba(255, 0, 0, 128),
              };

              if time + 400 > action_time && time < action_time {
                let line = graphics::Mesh::new_line(
                  ctx,
                  &[src_pos, tgt_pos],
                  20.0,
                  color
                ).unwrap();
                graphics::draw(
                  ctx,
                  &line,
                  graphics::DrawParam::default()
                ).unwrap()
              } else if time > action_time && time < action_time + 400 {
                graphics::draw(
                  ctx,
                  &self.assets.after_attack_effect,
                  graphics::DrawParam::default().dest(tgt_pos).color(color)
                ).unwrap();
              }
            }
          }
        }
      }
    }

    // FIXME: This could certainly be more efficient by not checking every single pattern note
    for pattern_note in &self.pattern {
      let x = (pattern_note.time as f32)/1000.0 * spacing_per_second - completion_offset_x + now_line_x;
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
          graphics::DrawParam::default().dest(Point2::new(x, y))
        ).unwrap();
      }
    }

    if self.sink.is_paused() {
      let text = graphics::Text::new(("Paused - press enter", self.assets.font, 75.0));
      let x = (window.w - text.width(ctx) as f32)/2.0;
      graphics::draw(
        ctx,
        &text,
        graphics::DrawParam::default().dest(Point2::new(x, 50.0))
      ).unwrap();
    }

    graphics::present(ctx)
  }

  fn key_down_event(
    &mut self,
    ctx: &mut Context,
    keycode: KeyCode,
    _keymods: KeyMods,
    repeat: bool
  ) {
    if repeat { return; }

    if self.sink.is_paused() {
      match keycode {
        KeyCode::Escape => event::quit(ctx),
        KeyCode::Return => self.sink.play(),
        _ => {}
      }
    } else {
      // TODO: Is the play_offset here slightly off because of time elapsed since last update()?
      match keycode {
        KeyCode::Escape => event::quit(ctx),
        KeyCode::Return => self.sink.pause(),
        KeyCode::Up | KeyCode::Right | KeyCode::Down | KeyCode::Left => {
          self.relative_pitch_input = Some(RelativePitchInput {
            direction: match keycode {
              KeyCode::Up => NavDirection::Up,
              KeyCode::Down => NavDirection::Down,
              KeyCode::Left => NavDirection::Left,
              KeyCode::Right => NavDirection::Right,
              _ => unreachable!()
            },
            relative_pitch: match keycode {
              KeyCode::Up | KeyCode::Right => RelativePitch::High,
              KeyCode::Down | KeyCode::Left => RelativePitch::Low,
              _ => unreachable!()
            },
            time: self.time.load(Ordering::Relaxed),
          })
        },
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
