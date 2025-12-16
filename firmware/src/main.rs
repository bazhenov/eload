//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

#![deny(unsafe_code)]
#![no_std]
#![no_main]

use core::{fmt::Write as _, mem};
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

    let mut current_screen = Screens::Main(MainScreen);
    current_screen.draw(&mut lcd, &state);

    // Application event loop
    loop {
        // Scanning inputs
        let encoder_event = encoder.scan();
        let encoder_button_event = encoder_button.scan();

        let events = [
            encoder_event.map(InputEvent::Encoder),
            encoder_button_event.map(InputEvent::EncoderButton),
        ];

        // Updating app state beased on events
        for e in events.into_iter().flatten() {
            current_screen.handle_input(e, &mut state);
        }

        // Navigation between panels and updating UI
        if encoder_button_event == Some(LongPressButtonValue::LongPress) {
            current_screen = match current_screen {
                Screens::Main(_) => Screens::Ticks(TicksScreen::default()),
                Screens::Ticks(_) => Screens::Main(MainScreen),
            };
            current_screen.draw(&mut lcd, &state);
        } else {
            current_screen.update(&mut lcd, &state);
        }

        // Controlling external world
        if state.tick() {
            led.toggle();
        }

        block!(timer.wait()).unwrap();
    }
}

#[derive(PartialEq, Eq)]
enum InputEvent {
    Encoder(EncoderValue),
    EncoderButton(LongPressButtonValue),
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

enum Screens {
    Main(MainScreen),
    Ticks(TicksScreen),
}

impl Screen for Screens {
    type State = State;
    type Ui = Ui;
    type InputEvent = InputEvent;

    fn draw(&mut self, ui: &mut Ui, state: &State) {
        match self {
            Screens::Main(p) => p.draw(ui, state),
            Screens::Ticks(p) => p.draw(ui, state),
        }
    }

    fn handle_input(&mut self, inputs: InputEvent, state: &mut State) {
        match self {
            Screens::Main(p) => p.handle_input(inputs, state),
            Screens::Ticks(p) => p.handle_input(inputs, state),
        };
    }

    fn update(&mut self, ui: &mut Ui, state: &State) {
        match self {
            Screens::Main(p) => p.update(ui, state),
            Screens::Ticks(p) => p.update(ui, state),
        }
    }
}

trait Screen {
    type State;
    type Ui;
    type InputEvent;

    fn draw(&mut self, ui: &mut Self::Ui, state: &Self::State);
    fn handle_input(&mut self, _inputs: Self::InputEvent, _state: &mut Self::State) {}
    fn update(&mut self, _ui: &mut Ui, _state: &Self::State) {}
}

struct TicksScreen {
    redraw_ticks: bool,
}

impl Default for TicksScreen {
    fn default() -> Self {
        Self { redraw_ticks: true }
    }
}

impl Screen for TicksScreen {
    type State = State;
    type Ui = Ui;
    type InputEvent = InputEvent;

    fn handle_input(&mut self, ev: Self::InputEvent, state: &mut Self::State) {
        match ev {
            InputEvent::Encoder(EncoderValue::Cw) => state.increase_freq(),
            InputEvent::Encoder(EncoderValue::Ccw) => state.decrease_freq(),
            _ => {}
        }
    }

    fn draw(&mut self, ui: &mut Ui, state: &State) {
        ui.lcd.clear(&mut ui.delay).unwrap();
        ui.lcd.write_str("Ticks Panel", &mut ui.delay).unwrap();
        self.update(ui, state);
    }

    fn update(&mut self, ui: &mut Ui, state: &State) {
        if mem::take(&mut self.redraw_ticks) {
            let mut data = String::<4>::new();
            write!(&mut data, "{:4}", state.ticks_max).unwrap();
            let (cols, _) = ui.lcd.display_size().get();
            ui.lcd.set_cursor_xy((cols - 4, 0), &mut ui.delay).unwrap();
            ui.lcd.write_str(data.as_str(), &mut ui.delay).unwrap();
        }
    }
}

struct MainScreen;

impl Screen for MainScreen {
    type State = State;
    type Ui = Ui;
    type InputEvent = InputEvent;

    fn draw(&mut self, ui: &mut Self::Ui, _state: &Self::State) {
        ui.lcd.clear(&mut ui.delay).unwrap();
        ui.lcd.write_str("Main Panel", &mut ui.delay).unwrap();
    }
}
