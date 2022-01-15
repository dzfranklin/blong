#![no_std]
#![no_main]

use blong as _; // global logger + panicking-behavior + memory layout

// This logs decimals seconds
defmt::timestamp!("{=u32:us}", app::monotonics::MonoDefault::now().ticks());

#[rtic::app(
    device = hal::pac,
    peripherals = true,
    dispatchers = [TIMER1]
)]
mod app {
    use blong::timer::MonoTimer;
    #[allow(unused_imports)]
    use defmt::{debug, error, info, warn, Format};

    use hal::gpio::{Level, Output, Pin};
    use nrf52840_hal as hal;
    use nrf52840_hal::gpio::PushPull;

    use hal::gpiote::Gpiote;
    use hal::pac::TIMER0;
    use hal::prelude::*;

    // A monotonic timer to enable scheduling in RTIC
    #[monotonic(binds = TIMER0, default = true)]
    type MonoDefault = MonoTimer<TIMER0>;

    #[shared]
    struct Shared {
        indicator_led: Pin<Output<PushPull>>,
    }

    #[local]
    struct Local {
        gpiote: Gpiote,
    }

    #[init]
    fn init(mut cx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Setup timers
        let mono_default = MonoTimer::new(cx.device.TIMER0);

        // Setup sleep
        //   Set the ARM SLEEPONEXIT bit to go to sleep after handling interrupts.
        //   See https://developer.arm.com/docs/100737/0100/power-management/sleep-mode/sleep-on-exit-bit
        cx.core.SCB.set_sleepdeep();

        let gpiote = Gpiote::new(cx.device.GPIOTE);
        let port0 = hal::gpio::p0::Parts::new(cx.device.P0);

        // Setup builtin button
        gpiote
            .channel0()
            .input_pin(&port0.p0_29.into_pullup_input().degrade())
            .hi_to_lo()
            .enable_interrupt();

        // Setup builtin indicator
        let indicator_led = port0.p0_06.into_push_pull_output(Level::Low).degrade();

        // Spawn task, runs right after init finishes
        startup::spawn().unwrap();

        (
            Shared { indicator_led },
            Local { gpiote },
            init::Monotonics(mono_default),
        )
    }

    // Background task, runs whenever no other tasks are running
    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            // Wait For Interrupt is used instead of a busy-wait loop
            // to allow MCU to sleep between interrupts, since we set
            // SLEEPONEXIT in init.
            // https://developer.arm.com/documentation/ddi0406/c/Application-Level-Architecture/Instruction-Details/Alphabetical-list-of-instructions/WFI
            rtic::export::wfi();
        }
    }

    // Software task, also not bound to a hardware interrupt
    #[task]
    fn startup(_cx: startup::Context) {
        info!("Starting up");
    }

    #[task(
        binds = GPIOTE,
        shared = [indicator_led],
        local = [gpiote, btn_toggled_to: bool = false]
    )]
    fn on_gpiote(cx: on_gpiote::Context) {
        let gpiote = cx.local.gpiote;
        let btn_toggled_to = cx.local.btn_toggled_to;
        let mut indicator_led = cx.shared.indicator_led;

        if gpiote.channel0().is_event_triggered() {
            debug!("Button press");
            *btn_toggled_to = !*btn_toggled_to;
            indicator_led.lock(|indicator_led| {
                if *btn_toggled_to {
                    indicator_led.set_high().unwrap();
                } else {
                    indicator_led.set_low().unwrap();
                }
            });
        }

        gpiote.reset_events();
    }
}
