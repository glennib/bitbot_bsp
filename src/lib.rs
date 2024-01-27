#![no_std]

pub use light_sensors::*;
pub use ultrasonic::*;
pub use wheels::*;

mod wheels {
    use embassy_nrf::{
        peripherals::{P0_01, P0_10, P0_12, P1_02, PWM0},
        pwm::{Prescaler, SimplePwm},
    };

    pub struct Wheels {
        pwm: SimplePwm<'static, PWM0>,
    }

    impl Wheels {
        pub const MAX_SPEED: i8 = 100;
        const MAX_DUTY: u16 = 0x7FFF;
        pub fn new(pwm: PWM0, p8: P0_10, p12: P0_12, p14: P0_01, p16: P1_02) -> Self {
            let pwm = SimplePwm::new_4ch(pwm, p8, p16, p12, p14);

            pwm.set_max_duty(Self::MAX_DUTY);
            pwm.set_prescaler(Prescaler::Div4);
            pwm.set_period(440);
            pwm.enable();
            Self { pwm }
        }

        pub fn set_speed(&mut self, left: i8, right: i8) {
            let duties = |speed: i8| {
                let reverse = speed < 0;
                let speed = speed.unsigned_abs().min(Self::MAX_SPEED as u8);
                let duty = u16::try_from(
                    (u32::from(speed) * u32::from(Self::MAX_DUTY)) / (Self::MAX_SPEED as u32),
                )
                .unwrap();
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
}

mod ultrasonic {
    use core::ops::{Div, Mul};
    use defmt::Format;
    use embassy_futures::select::{select, Either};
    use embassy_nrf::{
        gpio::{Input, Level, Output, OutputDrive, Pin, Pull},
        peripherals::P0_13,
    };
    use embassy_time::{Duration, Instant, Timer};

    pub struct Ultrasonic {
        pin: P0_13,
    }

    #[derive(Copy, Clone, Format)]
    pub struct Micrometers(pub u32);

    impl Micrometers {
        pub const fn as_millimeters(self) -> u32 {
            self.0 / 1_000
        }
        pub const fn as_centimeters(self) -> u32 {
            self.0 / 10_000
        }
        pub const fn as_decimeters(self) -> u32 {
            self.0 / 100_000
        }
        pub const fn as_meters(self) -> u32 {
            self.0 / 1_000_000
        }
        pub const fn from_millimeters(d: u32) -> Self {
            Self(d * 1_000)
        }
        pub const fn from_centimeters(d: u32) -> Self {
            Self(d * 10_000)
        }
        pub const fn from_decimeters(d: u32) -> Self {
            Self(d * 100_000)
        }
        pub const fn from_meters(d: u32) -> Self {
            Self(d * 1_000_000)
        }
        pub const fn from_duration(d: Duration, speed_mps: u32) -> Self {
            #[allow(clippy::cast_possible_truncation)]
            Self(speed_mps * (d.as_micros() as u32))
        }
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
        const SPEED_OF_SOUND_MPS: u32 = 343;
        // const MIN_DISTANCE: Micrometers = Micrometers::from_centimeters(2);
        const MAX_DISTANCE: Micrometers = Micrometers::from_meters(4);
        // const MIN_DURATION: Duration =
        //     Micrometers(Self::MIN_DISTANCE.0 * 2).into_duration(Self::SPEED_OF_SOUND_MPS);
        const MAX_DURATION: Duration =
            Micrometers(Self::MAX_DISTANCE.0 * 2).into_duration(Self::SPEED_OF_SOUND_MPS);
        pub fn new(p15: P0_13) -> Self {
            Self { pin: p15 }
        }

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
}

mod light_sensors {
    use defmt::Format;
    use embassy_nrf::{
        interrupt,
        peripherals::{P0_03, P0_04, SAADC},
        saadc::{ChannelConfig, Config, InterruptHandler, Saadc},
    };

    pub struct LightSensors {
        adc: Saadc<'static, 2>,
    }

    #[derive(Copy, Clone, Format)]
    pub struct LightValues {
        pub right: i16,
        pub left: i16,
    }

    impl LightSensors {
        pub async fn new(
            saadc: SAADC,
            p1: P0_03,
            p2: P0_04,
            irq: impl interrupt::typelevel::Binding<interrupt::typelevel::SAADC, InterruptHandler>
                + 'static,
        ) -> Self {
            let config = Config::default();
            let right = ChannelConfig::single_ended(p1);
            let left = ChannelConfig::single_ended(p2);
            let adc = Saadc::new(saadc, irq, config, [right, left]);
            adc.calibrate().await;
            Self { adc }
        }

        pub async fn read(&mut self) -> LightValues {
            let mut buffer = [0; 2];
            self.adc.sample(&mut buffer).await;
            LightValues {
                right: buffer[0],
                left: buffer[1],
            }
        }
    }
}
