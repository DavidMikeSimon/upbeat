use std::{
  convert::TryInto,
  sync::{Arc, atomic::{AtomicU32, Ordering}},
  time::Duration,
};

use rodio::{Sample, Source};

#[derive(Clone, Debug)]
pub struct CountingSource<I> {
  input: I,
  samples_till_next_ms: i32,
  ms_elapsed: Arc<AtomicU32>
}

impl<I> CountingSource<I>
where
  I: Source,
  I::Item: Sample,
{
  pub fn new(source: I) -> (CountingSource<I>, Arc<AtomicU32>) {
    let arc = Arc::new(AtomicU32::new(0));
    let initial_sample_rate = source.sample_rate();
    let mut counting_source = CountingSource {
      input: source,
      samples_till_next_ms: (initial_sample_rate/1000).try_into().unwrap(),
      ms_elapsed: arc.clone()
    };
    counting_source.reset_samples_till_next_ms();
    (counting_source, arc)
  }

  fn reset_samples_till_next_ms(&mut self) {
    self.samples_till_next_ms = (self.input.sample_rate()/1000 * (self.input.channels() as u32)).try_into().unwrap();
  }
}


impl<I> Iterator for CountingSource<I>
where
  I: Source,
  I::Item: Sample,
{
  type Item = I::Item;

  #[inline]
  fn next(&mut self) -> Option<I::Item> {
    let item = self.input.next();
    if let Some(_) = item {
      self.samples_till_next_ms -= 1;
      if self.samples_till_next_ms <= 0 {
        self.reset_samples_till_next_ms();
        self.ms_elapsed.fetch_add(1, Ordering::Relaxed);
      }
    }
    item
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.input.size_hint()
  }
}

impl<I> Source for CountingSource<I>
where
  I: Source,
  I::Item: Sample,
{
  #[inline]
  fn current_frame_len(&self) -> Option<usize> {
    self.input.current_frame_len()
  }

  #[inline]
  fn channels(&self) -> u16 {
    self.input.channels()
  }

  #[inline]
  fn sample_rate(&self) -> u32 {
    self.input.sample_rate()
  }

  #[inline]
  fn total_duration(&self) -> Option<Duration> {
    self.input.total_duration()
  }
}
