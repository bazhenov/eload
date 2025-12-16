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

pub struct LongPressButton<const CONTROL_RATE_HZ: u32, P> {
    pin: P,
    state: LongPressButtonState,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum LongPressButtonValue {
    Press,
    LongPress,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum LongPressButtonState {
    Depressed,
    Candidate(u32),
    Pressed(u32),
    StillPressed, // needed so that second press will not be registered after LongPress detected
}

const fn max(a: u32, b: u32) -> u32 {
    if a > b { a } else { b }
}

impl<const CONTROL_RATE_HZ: u32, P: InputPin> LongPressButton<CONTROL_RATE_HZ, P> {
    const LONGPRESS_TICKS: u32 = max(1, CONTROL_RATE_HZ); // 1 second
    const DEBOUNCE_TICKS: u32 = max(1, CONTROL_RATE_HZ / 1000); // 1ms

    pub fn new(pin: P) -> Self {
        Self {
            pin,
            state: LongPressButtonState::Depressed,
        }
    }

    pub fn scan(&mut self) -> Option<LongPressButtonValue> {
        use LongPressButtonState::*;
        use LongPressButtonValue::*;

        let pressed = self.pin.is_low().unwrap_infallible();
        let (state, value) = match (pressed, self.state) {
            (true, Depressed) => (Candidate(0), None),
            (true, Candidate(i)) if i > Self::DEBOUNCE_TICKS => (Pressed(0), None),
            (true, Candidate(i)) => (Candidate(i + 1), None),
            (true, Pressed(i)) if i > Self::LONGPRESS_TICKS => (StillPressed, Some(LongPress)),
            (true, Pressed(i)) => (Pressed(i + 1), None),
            (true, StillPressed) => (StillPressed, None),
            (false, Pressed(_)) => (Depressed, Some(Press)),
            (false, _) => (Depressed, None),
        };
        self.state = state;
        value
    }
}

pub struct Encoder<A, B> {
    a_pin: A,
    b_pin: B,
    previous_state: (bool, bool),
}

#[derive(PartialEq, Eq)]
pub enum EncoderValue {
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
    pub fn scan(&mut self) -> Option<EncoderValue> {
        // Both signals are active low
        let a = self.a_pin.is_low().unwrap_infallible();
        let b = self.b_pin.is_low().unwrap_infallible();

        let prev = self.previous_state;
        self.previous_state = (a, b);

        if (a, b) == (true, true) {
            match prev {
                (false, true) => Some(EncoderValue::Cw),
                (true, false) => Some(EncoderValue::Ccw),
                _ => None,
            }
        } else {
            None
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
