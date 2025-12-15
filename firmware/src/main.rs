//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

#![deny(unsafe_code)]
#![no_std]
#![no_main]

use core::fmt::Write as _;
use cortex_m_rt::entry;
use eload::{Encoder, EncoderValue, Inputs, Led, LongPressButton, LongPressButtonValue};
use hd44780_driver::{
    HD44780,
    bus::{EightBitBus, EightBitBusPins},
    charset::{CharsetUniversal, Fallback},
    memory_map::{MemoryMap1602, StandardMemoryMap},
    setup::DisplayOptions8Bit,
};
use heapless::String;
use nb::block;
use panic_rtt_target as _;
use rtt_target::rtt_init_default;
use stm32f1xx_hal::{
    gpio::{Output, Pin},
    pac::{self, TIM1},
    prelude::*,
    timer::{DelayUs, Timer},
};

const CONTROL_RATE_HZ: u32 = 1000;
type EncoderLed<T> = Led<true, CONTROL_RATE_HZ, T>;
type EncoderButton<T> = LongPressButton<CONTROL_RATE_HZ, T>;

#[entry]
fn main() -> ! {
    let channels = rtt_init_default!();
    rtt_target::set_print_channel(channels.up.0);

    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();

    // Acquire the GPIOC peripheral
    let mut gpioa = dp.GPIOA.split(&mut rcc);
    let mut gpioc = dp.GPIOC.split(&mut rcc);
    let mut gpiob = dp.GPIOB.split(&mut rcc);

    // Write to an LCD
    // {
    let mut delay = dp.TIM1.delay_us(&mut rcc);

    let rs = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);
    let en = gpioa.pa9.into_push_pull_output(&mut gpioa.crh);

    let d0 = gpioa.pa0.into_push_pull_output(&mut gpioa.crl);
    let d1 = gpioa.pa1.into_push_pull_output(&mut gpioa.crl);
    let d2 = gpioa.pa2.into_push_pull_output(&mut gpioa.crl);
    let d3 = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
    let d4 = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    let d5 = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);
    let d6 = gpioa.pa6.into_push_pull_output(&mut gpioa.crl);
    let d7 = gpioa.pa7.into_push_pull_output(&mut gpioa.crl);

    let mut lcd = HD44780::new(
        DisplayOptions8Bit::new(MemoryMap1602::new()).with_pins(EightBitBusPins {
            rs,
            en,
            d0,
            d1,
            d2,
            d3,
            d4,
            d5,
            d6,
            d7,
        }),
        &mut delay,
    )
    .ok()
    .unwrap();

    lcd.reset(&mut delay).unwrap();
    lcd.clear(&mut delay).unwrap();

    let mut lcd = Ui { lcd, delay };

    let pb5 = gpiob.pb5.into_pull_up_input(&mut gpiob.crl);
    let pb10 = gpiob.pb10.into_pull_up_input(&mut gpiob.crh);
    let pb11 = gpiob.pb11.into_pull_up_input(&mut gpiob.crh);
    let mut encoder = Encoder::new(pb10, pb11);
    let mut encoder_button = EncoderButton::new(pb5);

    let led_pin = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    let mut led = EncoderLed::new(led_pin);

    // Configure the syst timer to trigger an update every second
    let mut timer = Timer::syst(cp.SYST, &rcc.clocks).counter_hz();

    timer.start(CONTROL_RATE_HZ.Hz()).unwrap();

    let mut state = State::default();

    loop {
        let inputs = Inputs {
            encoder: encoder.scan(),
            encoder_button: encoder_button.scan(),
        };

        state.handle_inputs(inputs);

        if state.request_redraw {
            lcd.draw(&state);
            state.request_redraw = false;
        }

        if state.tick() {
            led.toggle();
        }

        block!(timer.wait()).unwrap();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    MainPanel,
    TicksPanel,
}

pub struct State {
    ticks_max: u32,
    tick: u32,
    panel: Panel,
    request_redraw: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ticks_max: 20,
            tick: 0,
            panel: Panel::MainPanel,
            request_redraw: true,
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

    fn handle_inputs(&mut self, mut inputs: Inputs) {
        if let Some(b) = inputs.encoder_button.take() {
            match b {
                LongPressButtonValue::Press => {}
                LongPressButtonValue::LongPress => {
                    self.panel = if self.panel == Panel::MainPanel {
                        Panel::TicksPanel
                    } else {
                        Panel::MainPanel
                    };
                    self.request_redraw = true;
                }
            }
        }
        if let Some(input) = inputs.encoder.take() {
            match input {
                EncoderValue::Cw => self.increase_freq(),
                EncoderValue::Ccw => self.decrease_freq(),
            }
            self.request_redraw = true;
        }
    }
}

#[allow(clippy::type_complexity)]
struct Ui {
    lcd: HD44780<
        EightBitBus<
            Pin<'A', 8, Output>,
            Pin<'A', 9, Output>,
            Pin<'A', 0, Output>,
            Pin<'A', 1, Output>,
            Pin<'A', 2, Output>,
            Pin<'A', 3, Output>,
            Pin<'A', 4, Output>,
            Pin<'A', 5, Output>,
            Pin<'A', 6, Output>,
            Pin<'A', 7, Output>,
        >,
        StandardMemoryMap<16, 2>,
        Fallback<CharsetUniversal, 32>,
    >,
    delay: DelayUs<TIM1>,
}

impl Ui {
    fn draw(&mut self, state: &State) {
        self.lcd.clear(&mut self.delay).unwrap();
        match state.panel {
            Panel::MainPanel => {
                self.lcd.set_cursor_pos(0, &mut self.delay).unwrap();
                self.lcd.write_str("Main Panel", &mut self.delay).unwrap();
            }
            Panel::TicksPanel => {
                self.lcd.set_cursor_pos(0, &mut self.delay).unwrap();
                self.lcd.write_str("Ticks Panel", &mut self.delay).unwrap();
                let mut data = String::<4>::new();
                write!(&mut data, "{:4}", state.ticks_max).unwrap();
                let (cols, _) = self.lcd.display_size().get();
                self.lcd
                    .set_cursor_xy((cols - 4, 0), &mut self.delay)
                    .unwrap();
                self.lcd.write_str(data.as_str(), &mut self.delay).unwrap();
            }
        }
    }
}
