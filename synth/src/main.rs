extern crate anyhow;
extern crate clap;
extern crate cpal;

mod input;
use crate::input::*;

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SizedSample, I24, U24,
};
use cpal::{FromSample, Sample};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // read args
    // let args: Vec<String> = env::args().collect();
    // if args.len() != 1
    // {
    //     println!("Usage: cargo run <vol>");
    // }

    // let vol_str = &args[1];
    // let vol_res: Result<f32, _> = vol_str.parse();
    // let mut vol: f32 = 0.0;
    // match vol_res
    // {
    //     Ok(value) => 
    //     {
    //         println!("Volume: {}", value);
    //         vol = value;
    //     }

    //     Err(e) => 
    //     {
    //         eprintln!("Failed to parse float: {}", e);
    //     }
    // }
    
    // let stream = stream_setup_for(vol)?;
    // stream.play()?;
    // std::thread::sleep(std::time::Duration::from_millis(4000));
    
    // read in output stream
    // let host = cpal::default_host();

    // let device = host
    //     .default_input_device()
    //     .expect("No input device available");
    
    let device = find_spotify_device().unwrap();
    let config = device.default_input_config().unwrap();

    println!("Using device: {}", device.description()?);
    println!("Input config: {:?}", config);

    let rms_value = Arc::new(Mutex::new(0.0f32));
    let rms_clone = rms_value.clone();

    let err_fn = |err|
    {
        eprintln!("Stream error: {}", err);
    };

    let spotify_in_stream = build_stream(&device, &config.config()
        , rms_clone, err_fn)?;
    spotify_in_stream.play()?;

    loop
    {
        std::thread::sleep(Duration::from_millis(500));

        let rms = *rms_value.lock().unwrap();
        println!("RMS: {:.5}", rms);
    }
}

pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

pub struct Oscillator {
    pub sample_rate: f32,
    pub waveform: Waveform,
    pub current_sample_index: f32,
    pub frequency_hz: f32,
    pub vol_gain: f32,
}

impl Oscillator {
    fn advance_sample(&mut self) {
        self.current_sample_index = (self.current_sample_index + 1.0) % self.sample_rate;
    }

    fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    fn calculate_sine_output_from_freq(&self, freq: f32) -> f32 {
        let two_pi = 2.0 * std::f32::consts::PI;
        self.vol_gain * (self.current_sample_index * freq * two_pi / self.sample_rate).sin()
    }

    fn is_multiple_of_freq_above_nyquist(&self, multiple: f32) -> bool {
        self.frequency_hz * multiple > self.sample_rate / 2.0
    }

    fn sine_wave(&mut self) -> f32 {
        self.advance_sample();
        self.calculate_sine_output_from_freq(self.frequency_hz)
    }

    fn generative_waveform(&mut self, harmonic_index_increment: i32, gain_exponent: f32) -> f32 {
        self.advance_sample();
        let mut output = 0.0;
        let mut i = 1;
        while !self.is_multiple_of_freq_above_nyquist(i as f32) {
            let mut gain = 1.0 / (i as f32).powf(gain_exponent);
            gain *= self.vol_gain;
            output += gain * self.calculate_sine_output_from_freq(self.frequency_hz * i as f32);
            i += harmonic_index_increment;
        }
        output
    }

    fn square_wave(&mut self) -> f32 {
        self.generative_waveform(2, 1.0)
    }

    fn saw_wave(&mut self) -> f32 {
        self.generative_waveform(1, 1.0)
    }

    fn triangle_wave(&mut self) -> f32 {
        self.generative_waveform(2, 2.0)
    }

    fn tick(&mut self) -> f32 {
        match self.waveform {
            Waveform::Sine => self.sine_wave(),
            Waveform::Square => self.square_wave(),
            Waveform::Saw => self.saw_wave(),
            Waveform::Triangle => self.triangle_wave(),
        }
    }
}

pub fn stream_setup_for(i_vol_gain: f32) -> Result<cpal::Stream, anyhow::Error>
{
    let (_host, device, config) = host_device_setup()?;

    match config.sample_format() {
        cpal::SampleFormat::I8 => make_stream::<i8>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::I16 => make_stream::<i16>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::I24 => make_stream::<I24>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::I32 => make_stream::<i32>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::I64 => make_stream::<i64>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::U8 => make_stream::<u8>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::U16 => make_stream::<u16>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::U24 => make_stream::<U24>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::U32 => make_stream::<u32>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::U64 => make_stream::<u64>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::F32 => make_stream::<f32>(&device, &config.into(), i_vol_gain),
        cpal::SampleFormat::F64 => make_stream::<f64>(&device, &config.into(), i_vol_gain),
        sample_format => Err(anyhow::Error::msg(format!(
            "Unsupported sample format '{sample_format}'"
        ))),
    }
}

pub fn host_device_setup(
) -> Result<(cpal::Host, cpal::Device, cpal::SupportedStreamConfig), anyhow::Error> {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    println!("Output device: {}", device.id()?);

    let config = device.default_output_config()?;
    println!("Default output config: {config:?}");

    Ok((host, device, config))
}

pub fn make_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    i_vol_gain: f32,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let num_channels = config.channels as usize;
    let mut oscillator = Oscillator {
        waveform: Waveform::Sine,
        sample_rate: config.sample_rate as f32,
        current_sample_index: 0.0,
        frequency_hz: 440.0,
        vol_gain: i_vol_gain,
    };
    let err_fn = |err| eprintln!("Error building output sound stream: {err}");

    let time_at_start = std::time::Instant::now();
    println!("Time at start: {time_at_start:?}");

    let stream = device.build_output_stream(
        config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            // for 0-1s play sine, 1-2s play square, 2-3s play saw, 3-4s play triangle_wave
            let time_since_start = std::time::Instant::now()
                .duration_since(time_at_start)
                .as_secs_f32();
            if time_since_start < 1.0 {
                oscillator.set_waveform(Waveform::Sine);
            } else if time_since_start < 2.0 {
                oscillator.set_waveform(Waveform::Triangle);
            } else if time_since_start < 3.0 {
                oscillator.set_waveform(Waveform::Square);
            } else if time_since_start < 4.0 {
                oscillator.set_waveform(Waveform::Saw);
            } else {
                oscillator.set_waveform(Waveform::Sine);
            }
            process_frame(output, &mut oscillator, num_channels)
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}

fn process_frame<SampleType>(
    output: &mut [SampleType],
    oscillator: &mut Oscillator,
    num_channels: usize,
) where
    SampleType: Sample + FromSample<f32>,
{
    for frame in output.chunks_mut(num_channels) {
        let value: SampleType = SampleType::from_sample(oscillator.tick());

        // copy the same value to all channels
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
