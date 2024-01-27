#![no_std]
#![no_main]

use bitbot_bsp::{LightSensors, Ultrasonic, Wheels};
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::Timer;
use embassy_nrf::{bind_interrupts, saadc};
use microbit_bsp::Microbit;
#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

fn map_speed(ideal: i8) -> i8 {
    const MIN: i16 = 20;
    const MAX: i16 = 100;
    const REST: i16 = 100 - MIN;
    const ZERO_LIMIT: i8 = 2;
    if (-ZERO_LIMIT..=ZERO_LIMIT).contains(&ideal) {
        return 0;
    }

    let reverse = ideal < 0;
    let ideal = i16::from(ideal.abs()).min(MAX);
    let speed = i8::try_from(MIN + (REST * ideal) / 100).unwrap();

    if reverse {
        -speed
    } else {
        speed
    }
}

#[embassy_executor::main]
async fn main(#[allow(unused_variables)] s: Spawner) {
    let board = Microbit::default();

    info!("Hello");

    let mut wheels = Wheels::new(board.pwm0, board.p8, board.p12, board.p14, board.p16);

    let mut sonar = Ultrasonic::new(board.p15);
    bind_interrupts!(struct Irq{
        SAADC => saadc::InterruptHandler;
    });
    let irq = Irq {};

    let mut light_sensors = LightSensors::new(board.saadc, board.p1, board.p2, irq).await;

    loop {
        let light = light_sensors.read().await;
        info!("light: {}", light);
        let speed = match sonar.measure_distance().await {
            Ok(Some(distance)) => {
                let distance = distance.as_centimeters();
                info!("distance: {} cm", distance);
                if distance <= 25 {
                    0
                } else if distance <= 50 {
                    let delta = distance - 25;
                    (50 + delta).min(75)
                } else {
                    75
                }
            }
            Ok(None) => 100,
            Err(()) => 0,
        };
        let speed = i16::try_from(speed).unwrap();
        info!("speed: {}", speed);
        let diff = light.right - light.left;
        info!("light_diff: {}", diff);
        let diff = (diff / 30).min(25);
        info!("scaled_light_diff: {}", diff);
        let left = speed + diff;
        let right = speed - diff;
        info!("left: {}, right: {}", left, right);
        let left = map_speed(left.try_into().unwrap());
        let right = map_speed(right.try_into().unwrap());
        info!("left: {}, right: {}", left, right);
        wheels.set_speed(left, right);

        Timer::after_millis(100).await;
    }
}
