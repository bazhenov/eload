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
use eload::{Encoder, EncoderValue, Inputs, Led, LongPressButton, LongPressButtonValue, State};
use hd44780_driver::{
    HD44780, bus::EightBitBusPins, memory_map::MemoryMap1602, setup::DisplayOptions8Bit,
};
use heapless::String;
use nb::block;
use panic_rtt_target as _;
use rtt_target::rtt_init_default;
use stm32f1xx_hal::{pac, prelude::*, timer::Timer};

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

    let mut inputs = Inputs::default();
    let mut state = State::default();

    lcd.clear(&mut delay).unwrap();

    loop {
        inputs.encoder = encoder.scan();
        inputs.encoder_button = encoder_button.scan();

        if let Some(input) = inputs.encoder.take() {
            match input {
                EncoderValue::Cw => {
                    state.increase_freq();
                    lcd.set_cursor_pos(0, &mut delay).unwrap();
                    lcd.write_str(" CW", &mut delay).unwrap();
                }
                EncoderValue::Ccw => {
                    state.decrease_freq();
                    lcd.set_cursor_pos(0, &mut delay).unwrap();
                    lcd.write_str("CCW", &mut delay).unwrap();
                }
            };

            let mut data = String::<4>::new();
            write!(&mut data, "{:4}", state.ticks_max).unwrap();
            let (cols, _) = lcd.display_size().get();
            lcd.set_cursor_xy((cols - 4, 0), &mut delay).unwrap();
            lcd.write_str(data.as_str(), &mut delay).unwrap();
        }

        if let Some(input) = inputs.encoder_button.take() {
            match input {
                LongPressButtonValue::Press => {
                    lcd.set_cursor_pos(0, &mut delay).unwrap();
                    lcd.write_str(" PR", &mut delay).unwrap();
                }
                LongPressButtonValue::LongPress => {
                    lcd.set_cursor_pos(0, &mut delay).unwrap();
                    lcd.write_str("LPR", &mut delay).unwrap();
                }
            }
        }

        if state.tick() {
            led.toggle();
        }

        block!(timer.wait()).unwrap();
    }
}

fn foo() {
    let menu_items = menuItem(
        "Settings",
        submenu((
            menuItem("S1", NullController),
            menuItem("S1", NullController),
        )),
    );
}

trait MenuController {
    fn process_inputs_and_redraw(&mut self, inputs: Inputs);
}

trait MenuChildren {
    fn process_inputs_and_redraw(&mut self, idx: u8, inputs: Inputs);
}

struct SubmenuController<C> {
    current_child: u8,
    child_active: bool,
    children: C,
}

struct NullController;

impl MenuController for NullController {
    fn process_inputs_and_redraw(&mut self, inputs: Inputs) {}
}

impl<C> MenuController for SubmenuController<C> {
    fn process_inputs_and_redraw(&mut self, inputs: Inputs) {}
}

struct MenuItem<C> {
    title: &'static str,
    controller: C,
}

const fn submenu<C: MenuChildren>(children: C) -> SubmenuController<C> {
    SubmenuController {
        current_child: 0,
        child_active: false,
        children,
    }
}

const fn menuItem<C: MenuController>(title: &'static str, controller: C) -> MenuItem<C> {
    MenuItem { title, controller }
}

impl MenuChildren for () {
    fn process_inputs_and_redraw(&mut self, idx: u8, inputs: Inputs) {}
}

impl<A: MenuController> MenuChildren for MenuItem<A> {
    fn process_inputs_and_redraw(&mut self, idx: u8, inputs: Inputs) {
        assert!(idx == 0);
        self.controller.process_inputs_and_redraw(inputs);
    }
}

impl<A: MenuController, B: MenuController> MenuChildren for (MenuItem<A>, MenuItem<B>) {
    fn process_inputs_and_redraw(&mut self, idx: u8, inputs: Inputs) {
        match idx {
            0 => self.0.controller.process_inputs_and_redraw(inputs),
            1 => self.1.controller.process_inputs_and_redraw(inputs),
            _ => panic!("Invalid idx {}", idx),
        }
    }
}
