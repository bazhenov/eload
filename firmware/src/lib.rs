#![no_std]

use core::convert::Infallible;
use unwrap_infallible::UnwrapInfallible;

pub trait InputPin: embedded_hal::digital::InputPin<Error = Infallible> {}
pub trait OutputPin: embedded_hal::digital::OutputPin<Error = Infallible> {}
pub trait StatefulOutputPin: embedded_hal::digital::StatefulOutputPin<Error = Infallible> {}

impl<T> InputPin for T where T: embedded_hal::digital::InputPin<Error = Infallible> {}
impl<T> OutputPin for T where T: embedded_hal::digital::OutputPin<Error = Infallible> {}
impl<T> StatefulOutputPin for T where T: embedded_hal::digital::StatefulOutputPin<Error = Infallible>
{}

pub struct Inputs {
    pub encoder: EncoderState,
}

pub struct State {
    pub ticks_max: u32,
    pub tick: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ticks_max: 20,
            tick: 0,
        }
    }
}

impl State {
    pub fn increase_freq(&mut self) {
        self.ticks_max = (self.ticks_max - 20).max(20);
    }

    pub fn decrease_freq(&mut self) {
        self.ticks_max = (self.ticks_max + 20).min(1000)
    }

    pub fn tick(&mut self) -> bool {
        if self.tick >= self.ticks_max {
            self.tick = 0;
            true
        } else {
            self.tick += 1;
            false
        }
    }
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            encoder: EncoderState::Idle,
        }
    }
}

pub struct Encoder<A, B> {
    a_pin: A,
    b_pin: B,
    previous_state: (bool, bool),
}

pub enum EncoderState {
    Idle,
    Cw,
    Ccw,
}

impl<A: InputPin, B: InputPin> Encoder<A, B> {
    pub fn new(a_pin: A, b_pin: B) -> Self {
        Self {
            a_pin,
            b_pin,
            previous_state: (false, false),
        }
    }
    pub fn scan(&mut self) -> EncoderState {
        // Both signals are active low
        let a = self.a_pin.is_low().unwrap_infallible();
        let b = self.b_pin.is_low().unwrap_infallible();

        let prev = self.previous_state;
        self.previous_state = (a, b);

        if (a, b) == (true, true) {
            match prev {
                (false, true) => EncoderState::Cw,
                (true, false) => EncoderState::Ccw,
                _ => EncoderState::Idle,
            }
        } else {
            EncoderState::Idle
        }
    }
}

pub struct Led<const ACTIVE_LOW: bool, const CONTROL_RATE_HZ: u32, P> {
    led_pin: P,
    cycles: u32,
}

impl<const ACTIVE_LOW: bool, const CONTROL_RATE_HZ: u32, P: OutputPin>
    Led<ACTIVE_LOW, CONTROL_RATE_HZ, P>
{
    pub fn new(led_pin: P) -> Self {
        Self { led_pin, cycles: 0 }
    }

    pub fn blink_short(&mut self) {
        self.cycles = self.cycles.max(25 * (CONTROL_RATE_HZ / 1000).max(1));
    }

    pub fn blink_long(&mut self) {
        self.cycles = self.cycles.max(100 * (CONTROL_RATE_HZ / 1000).max(1));
    }

    pub fn update(&mut self) {
        self.cycles = self.cycles.saturating_sub(1);
        let enabled = (self.cycles > 0) ^ ACTIVE_LOW;
        if enabled {
            self.led_pin.set_high().unwrap_infallible();
        } else {
            self.led_pin.set_low().unwrap_infallible();
        };
    }
}

impl<const ACTIVE_LOW: bool, const CONTROL_RATE_HZ: u32, P: StatefulOutputPin>
    Led<ACTIVE_LOW, CONTROL_RATE_HZ, P>
{
    pub fn toggle(&mut self) {
        self.led_pin.toggle().unwrap_infallible();
    }
}
