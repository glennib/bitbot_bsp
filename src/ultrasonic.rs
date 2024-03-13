use core::ops::{Div, Mul};
use defmt::Format;
use embassy_futures::select::{select, Either};
use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pin, Pull},
    peripherals::P0_13,
};
use embassy_time::{Duration, Instant, Timer};

/// Represents the ultrasonic sensor on the bitbot.
pub struct Ultrasonic {
    pin: P0_13,
}

/// Represents a distance with micrometer resolution.
#[derive(Copy, Clone, Format)]
pub struct Micrometers(pub u32);

impl Micrometers {
    #[must_use]
    pub const fn as_millimeters(self) -> u32 {
        self.0 / 1_000
    }
    #[must_use]
    pub const fn as_centimeters(self) -> u32 {
        self.0 / 10_000
    }
    #[must_use]
    pub const fn as_decimeters(self) -> u32 {
        self.0 / 100_000
    }
    #[must_use]
    pub const fn as_meters(self) -> u32 {
        self.0 / 1_000_000
    }
    #[must_use]
    pub const fn from_millimeters(d: u32) -> Self {
        Self(d * 1_000)
    }
    #[must_use]
    pub const fn from_centimeters(d: u32) -> Self {
        Self(d * 10_000)
    }
    #[must_use]
    pub const fn from_decimeters(d: u32) -> Self {
        Self(d * 100_000)
    }
    #[must_use]
    pub const fn from_meters(d: u32) -> Self {
        Self(d * 1_000_000)
    }
    /// Create a new Micrometers instance from a duration and speed.
    #[must_use]
    pub const fn from_duration(d: Duration, speed_mps: u32) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        Self(speed_mps * (d.as_micros() as u32))
    }
    /// Convert a Micrometers instance to a duration.
    #[must_use]
    pub const fn into_duration(self, speed_mps: u32) -> Duration {
        Duration::from_micros((self.0 as u64) / (speed_mps as u64))
    }
}

impl Div<u32> for Micrometers {
    type Output = Self;

    fn div(self, rhs: u32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Mul<u32> for Micrometers {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Ultrasonic {
    /// The speed of sound in meters per second.
    pub const SPEED_OF_SOUND_MPS: u32 = 343;
    // const MIN_DISTANCE: Micrometers = Micrometers::from_centimeters(2);
    const MAX_DISTANCE: Micrometers = Micrometers::from_meters(4);
    // const MIN_DURATION: Duration =
    //     Micrometers(Self::MIN_DISTANCE.0 * 2).into_duration(Self::SPEED_OF_SOUND_MPS);
    const MAX_DURATION: Duration =
        Micrometers(Self::MAX_DISTANCE.0 * 2).into_duration(Self::SPEED_OF_SOUND_MPS);
    #[must_use]
    pub fn new(p15: P0_13) -> Self {
        Self { pin: p15 }
    }

    /// Measures the distance to an object in front of the sensor.
    ///
    /// # Errors
    ///
    /// Produces Err(()) if an internal error occurs.
    ///
    /// # Returns
    ///
    /// Ok(Some([`Micrometers`])) if we get a response from the sensor.
    /// Ok(None) if we don't get a response from the sensor before the timeout.
    pub async fn measure_distance(&mut self) -> Result<Option<Micrometers>, ()> {
        let mut pin = Output::new(&mut self.pin, Level::Low, OutputDrive::HighDrive);
        Timer::after_micros(2).await;
        generate_pulse(&mut pin, Pulse::High, Duration::from_micros(10)).await;
        drop(pin);
        let mut pin = Input::new(&mut self.pin, Pull::None);
        let pulse = measure_pulse(&mut pin, Pulse::High, Self::MAX_DURATION).await;
        match pulse {
            PulseResult::Pulse(duration) => Ok(Some(
                Micrometers::from_duration(duration, Self::SPEED_OF_SOUND_MPS) / 2,
            )),
            PulseResult::AwaitOppositeTimeout => Err(()),
            PulseResult::AwaitEdgeTimeout => Ok(None),
        }
    }
}

#[derive(Copy, Clone)]
enum Pulse {
    #[allow(dead_code)]
    Low,
    High,
}

async fn generate_pulse<T: Pin>(pin: &mut Output<'_, T>, pulse: Pulse, duration: Duration) {
    match pulse {
        Pulse::Low => {
            pin.set_low();
        }
        Pulse::High => {
            pin.set_high();
        }
    }
    Timer::after(duration).await;
    match pulse {
        Pulse::Low => {
            pin.set_high();
        }
        Pulse::High => {
            pin.set_low();
        }
    }
}

enum PulseResult {
    Pulse(Duration),
    AwaitOppositeTimeout,
    AwaitEdgeTimeout,
}

async fn measure_pulse<T: Pin>(
    pin: &mut Input<'_, T>,
    pulse: Pulse,
    timeout: Duration,
) -> PulseResult {
    let opposite_timeout = Timer::after(timeout);
    if let Either::Second(()) = match pulse {
        Pulse::Low => select(pin.wait_for_falling_edge(), opposite_timeout).await,
        Pulse::High => select(pin.wait_for_rising_edge(), opposite_timeout).await,
    } {
        return PulseResult::AwaitOppositeTimeout;
    };

    let begin = Instant::now();
    let pulse_timeout = Timer::after(timeout);

    if let Either::Second(()) = match pulse {
        Pulse::Low => select(pin.wait_for_rising_edge(), pulse_timeout).await,
        Pulse::High => select(pin.wait_for_falling_edge(), pulse_timeout).await,
    } {
        return PulseResult::AwaitEdgeTimeout;
    }

    PulseResult::Pulse(begin.elapsed())
}
