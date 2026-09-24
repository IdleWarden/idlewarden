// SPDX-License-Identifier: MPL-2.0
use std::thread::sleep;
use std::time::Duration;

use idlewarden_plugin_api::InputCommand;

use crate::coords::Rect;
use crate::events::{encode, Event};
use crate::humanise::Jitter;
use crate::{Humanisation, InputBackend, InputError, KillSwitch};

/// Where encoded events go. The device is behind this so the sequencing above
/// it can be tested without `/dev/uinput`, which exists on one platform and
/// needs a permission even there.
pub trait EventSink: Send {
    fn emit(&mut self, events: &[Event]) -> Result<(), InputError>;
}

/// Mouse and keyboard through a virtual device (ADR-0019). The compositor sees
/// an ordinary input device, which is why this works under Wayland as well as
/// X11, and why the events are global exactly like `SendInput`.
pub struct UinputBackend<S: EventSink> {
    sink: S,
    window: Rect,
    screen: Rect,
    kill: KillSwitch,
    humanisation: Humanisation,
    jitter: Jitter,
}

impl<S: EventSink> UinputBackend<S> {
    pub fn new(
        sink: S,
        window: Rect,
        screen: Rect,
        kill: KillSwitch,
        humanisation: Humanisation,
    ) -> Self {
        let seed = (window.left as u64) << 32 ^ (window.top as u64) ^ 0x5A5A_A5A5;
        UinputBackend {
            sink,
            window,
            screen,
            kill,
            humanisation,
            jitter: Jitter::new(seed),
        }
    }

    fn checkpoint(&self) -> Result<(), InputError> {
        if self.kill.is_engaged() {
            return Err(InputError::KillSwitchEngaged);
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<(), InputError> {
        self.checkpoint()?;
        sleep(Duration::from_millis(
            self.jitter.delay_ms(self.humanisation),
        ));
        self.checkpoint()
    }
}

impl<S: EventSink> InputBackend for UinputBackend<S> {
    fn execute(&mut self, command: &InputCommand) -> Result<(), InputError> {
        self.checkpoint()?;

        if let InputCommand::Wait { ms } = command {
            sleep(Duration::from_millis(*ms));
            return self.checkpoint();
        }

        self.pause()?;
        let events = encode(command, self.window, self.screen)?;
        self.sink.emit(&events)
    }
}

#[cfg(target_os = "linux")]
pub use device::UinputDevice;

#[cfg(target_os = "linux")]
mod device {
    use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
    use evdev::{
        AbsInfo, AbsoluteAxisType, AttributeSet, EventType, InputEvent, Key, RelativeAxisType,
        UinputAbsSetup,
    };

    use super::EventSink;
    use crate::events::{
        Event, ABS_MAX, ABS_X, ABS_Y, BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, EV_ABS, EV_KEY, EV_REL,
    };
    use crate::InputError;

    /// The device the compositor sees. Creating it needs write access to
    /// `/dev/uinput`, which is root-only on a stock install: the app tells the
    /// user to add a udev rule rather than asking for root (ADR-0019).
    pub struct UinputDevice(VirtualDevice);

    impl UinputDevice {
        pub fn open() -> Result<Self, InputError> {
            let axis = AbsInfo::new(0, 0, ABS_MAX, 0, 0, 1);

            let mut keys = AttributeSet::<Key>::new();
            for code in pressable() {
                keys.insert(Key::new(code));
            }

            let mut wheel = AttributeSet::<RelativeAxisType>::new();
            wheel.insert(RelativeAxisType::REL_WHEEL);

            let device = VirtualDeviceBuilder::new()
                .map_err(reason)?
                .name("IdleWarden")
                .with_keys(&keys)
                .map_err(reason)?
                .with_relative_axes(&wheel)
                .map_err(reason)?
                .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_X, axis))
                .map_err(reason)?
                .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_Y, axis))
                .map_err(reason)?
                .build()
                .map_err(reason)?;

            Ok(UinputDevice(device))
        }
    }

    impl EventSink for UinputDevice {
        fn emit(&mut self, events: &[Event]) -> Result<(), InputError> {
            let batch: Vec<InputEvent> = events
                .iter()
                .map(|event| InputEvent::new(kind(event.kind), event.code, event.value))
                .collect();

            self.0.emit(&batch).map_err(reason)
        }
    }

    fn kind(kind: u16) -> EventType {
        match kind {
            EV_KEY => EventType::KEY,
            EV_REL => EventType::RELATIVE,
            EV_ABS => EventType::ABSOLUTE,
            _ => EventType::SYNCHRONIZATION,
        }
    }

    fn reason(error: std::io::Error) -> InputError {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return InputError::Backend(
                "/dev/uinput is not writable; add a udev rule for it, then log back in".to_owned(),
            );
        }
        InputError::Backend(error.to_string())
    }

    fn pressable() -> Vec<u16> {
        let mut codes = vec![BTN_LEFT, BTN_RIGHT, BTN_MIDDLE];
        let named = [
            "enter",
            "escape",
            "tab",
            "space",
            "backspace",
            "delete",
            "insert",
            "home",
            "end",
            "pageup",
            "pagedown",
            "left",
            "up",
            "right",
            "down",
            "shift",
            "ctrl",
            "alt",
        ];

        for letter in 'a'..='z' {
            codes.extend(crate::keys::evdev_key(&letter.to_string()));
        }
        for digit in '0'..='9' {
            codes.extend(crate::keys::evdev_key(&digit.to_string()));
        }
        for index in 1..=24 {
            codes.extend(crate::keys::evdev_key(&format!("f{index}")));
        }
        for name in named {
            codes.extend(crate::keys::evdev_key(name));
        }

        codes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{ABS_X, BTN_LEFT, EV_ABS, EV_KEY};
    use idlewarden_plugin_api::{Key, MouseButton, Point};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<Vec<Event>>>>);

    impl Recorder {
        fn batches(&self) -> Vec<Vec<Event>> {
            self.0.lock().expect("recorder").clone()
        }
    }

    impl EventSink for Recorder {
        fn emit(&mut self, events: &[Event]) -> Result<(), InputError> {
            self.0.lock().expect("recorder").push(events.to_vec());
            Ok(())
        }
    }

    const WINDOW: Rect = Rect {
        left: 0,
        top: 0,
        width: 1001,
        height: 1001,
    };

    const SCREEN: Rect = Rect {
        left: 0,
        top: 0,
        width: 1921,
        height: 1081,
    };

    fn backend(kill: KillSwitch) -> (UinputBackend<Recorder>, Recorder) {
        let recorder = Recorder::default();
        let instant = Humanisation {
            min_delay_ms: 0,
            max_delay_ms: 0,
        };
        (
            UinputBackend::new(recorder.clone(), WINDOW, SCREEN, kill, instant),
            recorder,
        )
    }

    #[test]
    fn a_click_reaches_the_device_as_one_batch() {
        let (mut backend, recorder) = backend(KillSwitch::new());

        backend
            .execute(&InputCommand::Click {
                at: Point { x: 0.5, y: 0.5 },
                button: MouseButton::Left,
            })
            .expect("clicks");

        let batches = recorder.batches();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0][0].kind, EV_ABS);
        assert_eq!(batches[0][0].code, ABS_X);
        assert!(batches[0]
            .iter()
            .any(|event| event.kind == EV_KEY && event.code == BTN_LEFT && event.value == 1));
    }

    #[test]
    fn the_kill_switch_stops_the_command_before_the_device_sees_it() {
        let kill = KillSwitch::new();
        let (mut backend, recorder) = backend(kill.clone());
        kill.engage();

        let error = backend
            .execute(&InputCommand::KeyPress {
                key: Key("a".to_owned()),
            })
            .expect_err("halted");

        assert!(matches!(error, InputError::KillSwitchEngaged));
        assert!(recorder.batches().is_empty());
    }

    #[test]
    fn a_wait_sleeps_without_emitting_anything() {
        let (mut backend, recorder) = backend(KillSwitch::new());

        backend
            .execute(&InputCommand::Wait { ms: 1 })
            .expect("waits");

        assert!(recorder.batches().is_empty());
    }

    #[test]
    fn a_key_the_device_has_no_code_for_emits_nothing() {
        let (mut backend, recorder) = backend(KillSwitch::new());

        let error = backend
            .execute(&InputCommand::KeyPress {
                key: Key("any key".to_owned()),
            })
            .expect_err("no such key");

        assert!(matches!(error, InputError::Backend(_)));
        assert!(
            recorder.batches().is_empty(),
            "half a key press is worse than none"
        );
    }
}
