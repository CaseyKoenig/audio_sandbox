extern crate anyhow;
extern crate cpal;

use std::sync::{Arc, Mutex};

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};


pub fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    rms_value: Arc<Mutex<f32>>,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream>
where
    f32: cpal::Sample,
{
    let channels = config.channels as usize;

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _|
        {
            let mut sum = 0.0f32;
            let mut count = 0;

            for frame in data.chunks(channels)
            {
                let sample: f32 = frame[0];
                sum += sample * sample;
                count += 1;
            }

            if count > 0
            {
                let rms = (sum / count as f32).sqrt();
                *rms_value.lock().unwrap() = rms;
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}

pub fn find_spotify_device() -> Result<cpal::Device, anyhow::Error>
{
    let host = cpal::default_host();

    for device in host.input_devices().expect("error getting input devices")
    {
        let desc = device.description().expect("could not get device desc");
        let name = desc.name();
        println!("device: {}", name);
        if name.to_lowercase().contains("pipewire")
        {
            println!("Using input device: {}", name);
            return Ok(device);
        }
    }
    return Err(anyhow::Error::msg("couldn't find spotify device"));
}

// pub fn find_spotify_config(device: &cpal::Device) -> 
//     Result<cpal::SupportedStreamConfig, anyhow::Error>
// {
//     for config in device.supported_input_configs()
//     {
//         let    
//     }
// }