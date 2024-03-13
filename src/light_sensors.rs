//! Contains utilities for reading the light sensors on the bitbot.

use defmt::Format;
use embassy_nrf::{
    interrupt,
    peripherals::{P0_03, P0_04, SAADC},
    saadc::{ChannelConfig, Config, InterruptHandler, Saadc},
};

/// Represents the left and right light sensors on the bitbot.
pub struct LightSensors {
    adc: Saadc<'static, 2>,
}

/// Represents the values read from the light sensors.
///
/// The meaning of the values are not documented, but correlate with the intensity of the light
/// detected.
#[derive(Copy, Clone, Format)]
pub struct LightValues {
    pub right: i16,
    pub left: i16,
}

impl LightSensors {
    /// Create a new [`LightSensors`] instance.
    ///
    /// # Arguments
    ///
    /// * `saadc`: An analog to digital converter instance from the micro:bit.
    /// * `p1`: The pin connected to the right light sensor.
    /// * `p2`: The pin connected to the left light sensor.
    /// * `irq`: 'Proof' of interrupt handler mapping.
    ///
    /// returns: [`LightSensors`]
    pub async fn new(
        saadc: SAADC,
        p1: P0_03,
        p2: P0_04,
        irq: impl interrupt::typelevel::Binding<interrupt::typelevel::SAADC, InterruptHandler> + 'static,
    ) -> Self {
        let config = Config::default();
        let right = ChannelConfig::single_ended(p1);
        let left = ChannelConfig::single_ended(p2);
        let adc = Saadc::new(saadc, irq, config, [right, left]);
        adc.calibrate().await;
        Self { adc }
    }

    /// Perform a read of the light sensors.
    pub async fn read(&mut self) -> LightValues {
        let mut buffer = [0; 2];
        self.adc.sample(&mut buffer).await;
        LightValues {
            right: buffer[0],
            left: buffer[1],
        }
    }
}
