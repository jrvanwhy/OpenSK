// Copyright 2019 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate arrayref;
extern crate byteorder;
#[cfg(feature = "std")]
extern crate core;
extern crate ctap2;
extern crate libtock;
extern crate subtle;
#[macro_use]
extern crate cbor;
extern crate crypto;

mod ctap;
mod usb_ctap_hid;

use core::cell::Cell;
#[cfg(feature = "debug_ctap")]
use core::fmt::Write;
use crypto::rng256::TockRng256;
use ctap::hid::{ChannelID, CtapHid, KeepaliveStatus, ProcessedPacket};
use ctap::status_code::Ctap2StatusCode;
use ctap::CtapState;
use libtock::buttons;
use libtock::buttons::ButtonState;
#[cfg(feature = "debug_ctap")]
use libtock::console::Console;
use libtock::lw::led;
use libtock::result::TockValue;
use libtock::syscalls;
use libtock::timer;
#[cfg(feature = "debug_ctap")]
use libtock::timer::Timer;
use libtock::timer::{Duration, StopAlarmError, Timestamp};

const KEEPALIVE_DELAY_MS: isize = 100;
const KEEPALIVE_DELAY: Duration<isize> = Duration::from_ms(KEEPALIVE_DELAY_MS);
const SEND_TIMEOUT: Duration<isize> = Duration::from_ms(1000);

fn main() {
    // Setup the timer with a dummy callback (we only care about reading the current time, but the
    // API forces us to set an alarm callback too).
    let mut with_callback = timer::with_callback(|_, _| {});
    let timer = with_callback.init().unwrap();

    // Setup USB driver.
    if !usb_ctap_hid::setup() {
        panic!("Cannot setup USB driver");
    }

    let mut rng = TockRng256 {};
    let mut ctap_state = CtapState::new(&mut rng, check_user_presence);
    let mut ctap_hid = CtapHid::new();

    let mut led_counter = 0;
    let mut last_led_increment = timer.get_current_clock();

    // Main loop. If CTAP1 is used, we register button presses for U2F while receiving and waiting.
    // The way TockOS and apps currently interact, callbacks need a yield syscall to execute,
    // making consistent blinking patterns and sending keepalives harder.
    loop {
        // Create the button callback, used for CTAP1.
        #[cfg(feature = "with_ctap1")]
        let button_touched = Cell::new(false);
        #[cfg(feature = "with_ctap1")]
        let mut buttons_callback = buttons::with_callback(|_button_num, state| {
            match state {
                ButtonState::Pressed => button_touched.set(true),
                ButtonState::Released => (),
            };
        });
        #[cfg(feature = "with_ctap1")]
        let mut buttons = buttons_callback.init().unwrap();
        #[cfg(feature = "with_ctap1")]
        // At the moment, all buttons are accepted. You can customize your setup here.
        for mut button in &mut buttons {
            button.enable().unwrap();
        }

        let mut pkt_request = [0; 64];
        let has_packet = match usb_ctap_hid::recv_with_timeout(&mut pkt_request, KEEPALIVE_DELAY) {
            Some(usb_ctap_hid::SendOrRecvStatus::Received) => {
                #[cfg(feature = "debug_ctap")]
                print_packet_notice("Received packet", &timer);
                true
            }
            Some(_) => panic!("Error receiving packet"),
            None => false,
        };

        let now = timer.get_current_clock();
        #[cfg(feature = "with_ctap1")]
        {
            if button_touched.get() {
                ctap_state.u2f_up_state.grant_up(now);
            }
            // Cleanup button callbacks. We miss button presses while processing though.
            // Heavy computation mostly follows a registered touch luckily. Unregistering
            // callbacks is important to not clash with those from check_user_presence.
            for mut button in &mut buttons {
                button.disable().unwrap();
            }
            drop(buttons);
            drop(buttons_callback);
        }

        // These calls are making sure that even for long inactivity, wrapping clock values
        // never randomly wink or grant user presence for U2F.
        ctap_state.check_disable_reset(Timestamp::<isize>::from_clock_value(now));
        ctap_hid.wink_permission = ctap_hid.wink_permission.check_expiration(now);

        if has_packet {
            let reply = ctap_hid.process_hid_packet(&pkt_request, now, &mut ctap_state);
            // This block handles sending packets.
            for mut pkt_reply in reply {
                let status = usb_ctap_hid::send_or_recv_with_timeout(&mut pkt_reply, SEND_TIMEOUT);
                match status {
                    None => {
                        #[cfg(feature = "debug_ctap")]
                        print_packet_notice("Sending packet timed out", &timer);
                        // TODO: reset the ctap_hid state.
                        // Since sending the packet timed out, we cancel this reply.
                        break;
                    }
                    Some(usb_ctap_hid::SendOrRecvStatus::Error) => panic!("Error sending packet"),
                    Some(usb_ctap_hid::SendOrRecvStatus::Sent) => {
                        #[cfg(feature = "debug_ctap")]
                        print_packet_notice("Sent packet", &timer);
                    }
                    Some(usb_ctap_hid::SendOrRecvStatus::Received) => {
                        #[cfg(feature = "debug_ctap")]
                        print_packet_notice("Received an UNEXPECTED packet", &timer);
                        // TODO: handle this unexpected packet.
                    }
                }
            }
        }

        let now = timer.get_current_clock();
        if let Some(wait_duration) = now.wrapping_sub(last_led_increment) {
            if wait_duration > KEEPALIVE_DELAY {
                // Loops quickly when waiting for U2F user presence, so the next LED blink
                // state is only set if enough time has elapsed.
                led_counter += 1;
                last_led_increment = now;
            }
        } else {
            // This branch means the clock frequency changed. This should never happen.
            led_counter += 1;
            last_led_increment = now;
        }

        if ctap_hid.wink_permission.is_granted(now) {
            wink_leds(led_counter);
        } else {
            #[cfg(not(feature = "with_ctap1"))]
            switch_off_leds();
            #[cfg(feature = "with_ctap1")]
            {
                if ctap_state.u2f_up_state.is_up_needed(now) {
                    // Flash the LEDs with an almost regular pattern. The inaccuracy comes from
                    // delay caused by processing and sending of packets.
                    blink_leds(led_counter);
                } else {
                    switch_off_leds();
                }
            }
        }
    }
}

#[cfg(feature = "debug_ctap")]
fn print_packet_notice(notice_text: &str, timer: &Timer) {
    let now_us =
        (Timestamp::<f64>::from_clock_value(timer.get_current_clock()).ms() * 1000.0) as u64;
    writeln!(
        Console::new(),
        "{} at {}.{:06} s",
        notice_text,
        now_us / 1_000_000,
        now_us % 1_000_000
    )
    .unwrap();
}

// Returns whether the keepalive was sent, or false if cancelled.
fn send_keepalive_up_needed(
    cid: ChannelID,
    timeout: Duration<isize>,
) -> Result<(), Ctap2StatusCode> {
    let keepalive_msg = CtapHid::keepalive(cid, KeepaliveStatus::UpNeeded);
    for mut pkt in keepalive_msg {
        let status = usb_ctap_hid::send_or_recv_with_timeout(&mut pkt, timeout);
        match status {
            None => {
                #[cfg(feature = "debug_ctap")]
                writeln!(Console::new(), "Sending a KEEPALIVE packet timed out").unwrap();
                // TODO: abort user presence test?
            }
            Some(usb_ctap_hid::SendOrRecvStatus::Error) => panic!("Error sending KEEPALIVE packet"),
            Some(usb_ctap_hid::SendOrRecvStatus::Sent) => {
                #[cfg(feature = "debug_ctap")]
                writeln!(Console::new(), "Sent KEEPALIVE packet").unwrap();
            }
            Some(usb_ctap_hid::SendOrRecvStatus::Received) => {
                // We only parse one packet, because we only care about CANCEL.
                let (received_cid, processed_packet) = CtapHid::process_single_packet(&pkt);
                if received_cid != &cid {
                    #[cfg(feature = "debug_ctap")]
                    writeln!(
                        Console::new(),
                        "Received a packet on channel ID {:?} while sending a KEEPALIVE packet",
                        received_cid,
                    )
                    .unwrap();
                    return Ok(());
                }
                match processed_packet {
                    ProcessedPacket::InitPacket { cmd, .. } => {
                        if cmd == CtapHid::COMMAND_CANCEL {
                            // We ignore the payload, we can't answer with an error code anyway.
                            #[cfg(feature = "debug_ctap")]
                            writeln!(Console::new(), "User presence check cancelled").unwrap();
                            return Err(Ctap2StatusCode::CTAP2_ERR_KEEPALIVE_CANCEL);
                        } else {
                            #[cfg(feature = "debug_ctap")]
                            writeln!(
                                Console::new(),
                                "Discarded packet with command {} received while sending a KEEPALIVE packet",
                                cmd,
                            )
                            .unwrap();
                        }
                    }
                    ProcessedPacket::ContinuationPacket { .. } => {
                        #[cfg(feature = "debug_ctap")]
                        writeln!(
                            Console::new(),
                            "Discarded continuation packet received while sending a KEEPALIVE packet",
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    Ok(())
}

struct Led0; impl led::LedIdx for Led0 { const IDX: usize = 0; }
struct Led1; impl led::LedIdx for Led1 { const IDX: usize = 1; }
struct Led2; impl led::LedIdx for Led2 { const IDX: usize = 2; }
struct Led3; impl led::LedIdx for Led3 { const IDX: usize = 3; }

static LED0: led::Led<Led0> = led::Led::new();
static LED1: led::Led<Led1> = led::Led::new();
static LED2: led::Led<Led2> = led::Led::new();
static LED3: led::Led<Led3> = led::Led::new();

fn blink_leds(pattern_seed: isize) {
    if pattern_seed.count_ones() & 1 != 0 {
        let _ = LED0.turn_on();
        let _ = LED1.turn_on();
        let _ = LED2.turn_on();
        let _ = LED3.turn_on();
    } else {
        let _ = LED0.turn_off();
        let _ = LED1.turn_off();
        let _ = LED2.turn_off();
        let _ = LED3.turn_off();
    }
}

fn wink_leds(pattern_seed: isize) {
    // This generates a "snake" pattern circling through the LEDs.
    // Fox example with 4 LEDs the sequence of lit LEDs will be the following.
    // 0 1 2 3
    // * *
    // * * *
    //   * *
    //   * * *
    //     * *
    // *   * *
    // *     *
    // * *   *
    // * *
    let count = 4;
    let a = (pattern_seed / 2) % count;
    let b = ((pattern_seed + 1) / 2) % count;
    let c = ((pattern_seed + 3) / 2) % count;

    let mut leds = [false; 4];
    for l in 0..4 {
        // On nRF52840-DK, logically swap LEDs 3 and 4 so that the order of LEDs form a circle.
        let k = match l {
            2 => 3,
            3 => 2,
            _ => l,
        };
        if k == a || k == b || k == c {
            leds[k as usize] = true;
        } else {
            leds[k as usize] = false;
        }
    }
    let _ = if leds[0] { LED0.turn_on() } else { LED0.turn_off() };
    let _ = if leds[1] { LED1.turn_on() } else { LED1.turn_off() };
    let _ = if leds[2] { LED2.turn_on() } else { LED2.turn_off() };
    let _ = if leds[3] { LED3.turn_on() } else { LED3.turn_off() };
}

fn switch_off_leds() {
    let _ = LED0.turn_off();
    let _ = LED1.turn_off();
    let _ = LED2.turn_off();
    let _ = LED3.turn_off();
}

fn check_user_presence(cid: ChannelID) -> Result<(), Ctap2StatusCode> {
    // The timeout is N times the keepalive delay.
    const TIMEOUT_ITERATIONS: isize = ctap::TOUCH_TIMEOUT_MS / KEEPALIVE_DELAY_MS;

    // First, send a keep-alive packet to notify that the keep-alive status has changed.
    send_keepalive_up_needed(cid, KEEPALIVE_DELAY)?;

    // Listen to the button presses.
    let button_touched = Cell::new(false);
    let mut buttons_callback = buttons::with_callback(|_button_num, state| {
        match state {
            ButtonState::Pressed => button_touched.set(true),
            ButtonState::Released => (),
        };
    });
    let mut buttons = buttons_callback.init().unwrap();
    // At the moment, all buttons are accepted. You can customize your setup here.
    for mut button in &mut buttons {
        button.enable().unwrap();
    }

    let mut keepalive_response = Ok(());
    for i in 0..TIMEOUT_ITERATIONS {
        blink_leds(i);

        // Setup a keep-alive callback.
        let keepalive_expired = Cell::new(false);
        let mut keepalive_callback = timer::with_callback(|_, _| {
            keepalive_expired.set(true);
        });
        let mut keepalive = keepalive_callback.init().unwrap();
        let keepalive_alarm = keepalive.set_alarm(KEEPALIVE_DELAY).unwrap();

        // Wait for a button touch or an alarm.
        syscalls::yieldk_for(|| button_touched.get() || keepalive_expired.get());

        // Cleanup alarm callback.
        match keepalive.stop_alarm(keepalive_alarm) {
            Ok(()) => (),
            Err(TockValue::Expected(StopAlarmError::AlreadyDisabled)) => {
                assert!(keepalive_expired.get())
            }
            Err(e) => panic!("Unexpected error when stopping alarm: {:?}", e),
        }

        // TODO: this may take arbitrary time. The keepalive_delay should be adjusted accordingly,
        // so that LEDs blink with a consistent pattern.
        if keepalive_expired.get() {
            // Do not return immediately, because we must clean up still.
            keepalive_response = send_keepalive_up_needed(cid, KEEPALIVE_DELAY);
        }

        if button_touched.get() || keepalive_response.is_err() {
            break;
        }
    }

    switch_off_leds();

    // Cleanup button callbacks.
    for mut button in &mut buttons {
        button.disable().unwrap();
    }

    // Returns whether the user was present.
    if keepalive_response.is_err() {
        keepalive_response
    } else if button_touched.get() {
        Ok(())
    } else {
        Err(Ctap2StatusCode::CTAP2_ERR_USER_ACTION_TIMEOUT)
    }
}
