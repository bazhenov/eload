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
use eload::{Encoder, EncoderValue, Led, LongPressButton, LongPressButtonValue};
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

    let mut panels = Panels::MainPanel(MainPanel);

    loop {
        let inputs = Inputs {
            encoder: encoder.scan(),
            encoder_button: encoder_button.scan(),
        };

        panels.handle_inputs(inputs, &mut state);

        if inputs.encoder.is_some() {
            panels = Panels::TicksPanel(TicksPanel {
                redraw_ticks: Some(()),
            });
        }

        panels.redraw(&mut lcd, &state);

        if state.tick() {
            led.toggle();
        }

        block!(timer.wait()).unwrap();
    }
}

pub struct Inputs {
    pub encoder: Option<EncoderValue>,
    pub encoder_button: Option<LongPressButtonValue>,
}

pub struct State {
    ticks_max: u32,
    tick: u32,
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

enum Panels {
    MainPanel(MainPanel),
    TicksPanel(TicksPanel),
}

impl UiPanel<State, Ui, Inputs> for Panels {
    fn draw(&mut self, ui: &mut Ui, state: &State) {
        match self {
            Panels::MainPanel(p) => p.draw(ui, state),
            Panels::TicksPanel(p) => p.draw(ui, state),
        }
    }

    fn handle_inputs(&mut self, inputs: Inputs, state: &mut State) {
        match self {
            Panels::MainPanel(p) => p.handle_inputs(inputs, state),
            Panels::TicksPanel(p) => p.handle_inputs(inputs, state),
        };
    }

    fn redraw(&mut self, ui: &mut Ui, state: &State) {
        match self {
            Panels::MainPanel(p) => p.redraw(ui, state),
            Panels::TicksPanel(p) => p.redraw(ui, state),
        }
    }
}

trait UiPanel<State, Ui, Inputs> {
    fn draw(&mut self, ui: &mut Ui, state: &State);
    fn handle_inputs(&mut self, inputs: Inputs, state: &mut State);
    fn redraw(&mut self, ui: &mut Ui, state: &State);
}

struct TicksPanel {
    redraw_ticks: Option<()>,
}

impl UiPanel<State, Ui, Inputs> for TicksPanel {
    fn draw(&mut self, ui: &mut Ui, state: &State) {
        ui.lcd.clear(&mut ui.delay).unwrap();
        ui.lcd.write_str("Ticks Panel", &mut ui.delay).unwrap();
        let mut panel = Self {
            redraw_ticks: Some(()),
        };
        panel.redraw(ui, state);
    }

    fn handle_inputs(&mut self, mut inputs: Inputs, state: &mut State) {
        if let Some(b) = inputs.encoder.take() {
            match b {
                EncoderValue::Cw => state.increase_freq(),
                EncoderValue::Ccw => state.decrease_freq(),
            }
        }
        if let Some(f) = inputs.encoder_button.take() {
            // match f {
            //     LongPressButtonValue::LongPress => state.panel = MainPan
            // }
        }
    }

    fn redraw(&mut self, ui: &mut Ui, state: &State) {
        if self.redraw_ticks.take().is_some() {
            let mut data = String::<4>::new();
            write!(&mut data, "{:4}", state.ticks_max).unwrap();
            let (cols, _) = ui.lcd.display_size().get();
            ui.lcd.set_cursor_xy((cols - 4, 0), &mut ui.delay).unwrap();
            ui.lcd.write_str(data.as_str(), &mut ui.delay).unwrap();
        }
    }
}

struct MainPanel;

impl UiPanel<State, Ui, Inputs> for MainPanel {
    fn draw(&mut self, ui: &mut Ui, state: &State) {
        ui.lcd.clear(&mut ui.delay).unwrap();
        ui.lcd.write_str("Main Panel", &mut ui.delay).unwrap();
    }

    fn handle_inputs(&mut self, mut inputs: Inputs, state: &mut State) {}

    fn redraw(&mut self, ui: &mut Ui, state: &State) {}
}
