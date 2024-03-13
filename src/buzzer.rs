//! Contains a function to create a new speaker instance interfacing the bitbot speaker.

use embassy_nrf::{
    peripherals::P0_02,
    pwm::{Instance, SimplePwm},
    Peripheral,
};
use microbit_bsp::speaker::PwmSpeaker;

/// Create a new speaker instance interfacing the bitbot speaker.
pub fn new<T: Instance>(
    pwm: impl Peripheral<P = T> + 'static,
    p0: P0_02,
) -> PwmSpeaker<'static, T> {
    let pwm = SimplePwm::new_1ch(pwm, p0);
    PwmSpeaker::new(pwm)
}
