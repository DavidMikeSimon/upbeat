extern crate openmpt;
extern crate rodio;

use std::{
  fs::File,
  thread,
  time::Duration,
};

use openmpt::module::{Module, Logger};
use rodio::{buffer::SamplesBuffer, Sink};

fn main() {
  let mut stream = File::open("music/LPChip - Wisdom Of Purity.it").expect("open mod file");

	let mut module = Module::create(&mut stream, Logger::None, &[]).unwrap();

  let device = rodio::default_output_device().unwrap();
  let sink = Sink::new(&device);

	let mut buffer = vec![0f32; 44100]; // 1 second at a time

	loop {
		let avail_samples = module.read_interleaved_float_stereo(
				44100, &mut buffer) << 1; // We're in interleaved stereo
		if avail_samples <= 0 { break; }

    let vec: Vec<f32> = buffer[..avail_samples].into();
    println!("APPENDING {:?}", &vec.len());
    let buffer = SamplesBuffer::new(2, 44100, vec);
    sink.append(buffer);
	}

  thread::sleep(Duration::from_millis(5000));
}
