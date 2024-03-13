use embassy_nrf::{
    peripherals::{P0_01, P0_10, P0_12, P1_02},
    pwm::{Instance, Prescaler, SimplePwm},
    Peripheral,
};

/// Represents the wheels on the bitbot.
pub struct Wheels<T: Instance> {
    pwm: SimplePwm<'static, T>,
}

impl<T: Instance> Wheels<T> {
    /// The maximum speed the wheels can be set to.
    pub const MAX_SPEED: i8 = 100;
    const MAX_DUTY: u16 = 0x7FFF;

    /// Create a new Wheels instance from an [`embassy_nrf::pwm::Instance`] and pins on the
    /// micro:bit.
    #[must_use]
    pub fn new(
        pwm: impl Peripheral<P = T> + 'static,
        p8: P0_10,
        p12: P0_12,
        p14: P0_01,
        p16: P1_02,
    ) -> Self {
        let pwm = SimplePwm::new_4ch(pwm, p8, p16, p12, p14);

        pwm.set_max_duty(Self::MAX_DUTY);
        pwm.set_prescaler(Prescaler::Div4);
        pwm.set_period(440);
        pwm.enable();
        Self { pwm }
    }

    /// Set the speed of the wheels.
    pub fn set_speed(&mut self, left: i8, right: i8) {
        let duties = |speed: i8| {
            let reverse = speed < 0;
            let speed = speed.unsigned_abs().min(Self::MAX_SPEED as u8);

            // SAFETY: MAX_SPEED * MAX_DUTY / MAX_SPEED is 0x7FFF, which is less than u16::MAX.
            let duty = unsafe {
                u16::try_from(
                    (u32::from(speed) * u32::from(Self::MAX_DUTY)) / (Self::MAX_SPEED as u32),
                )
                .unwrap_unchecked()
            };
            if reverse {
                (0, duty)
            } else {
                (duty, 0)
            }
        };
        let (f, r) = duties(left);
        self.pwm.set_duty(0, f);
        self.pwm.set_duty(1, r);
        let (f, r) = duties(right);
        self.pwm.set_duty(2, f);
        self.pwm.set_duty(3, r);
    }
}
