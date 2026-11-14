use collections::HashMap;

#[derive(Debug, Hash, PartialEq, Eq)]
pub(crate) enum SerialKind {
    DataDevice,
    InputMethod,
    MouseEnter,
    MousePress,
    KeyPress,
}

#[derive(Debug)]
struct SerialData {
    serial: u32,
}

impl SerialData {
    fn new(value: u32) -> Self {
        Self { serial: value }
    }
}

#[derive(Debug)]
/// Helper for tracking of different serial kinds.
pub(crate) struct SerialTracker {
    serials: HashMap<SerialKind, SerialData>,
    input_serial: u32,
}

impl SerialTracker {
    pub fn new() -> Self {
        Self {
            serials: HashMap::default(),
            input_serial: 0,
        }
    }

    pub fn update(&mut self, kind: SerialKind, value: u32) {
        if matches!(&kind, SerialKind::KeyPress | SerialKind::MousePress) {
            self.input_serial = value;
        }
        self.serials.insert(kind, SerialData::new(value));
    }

    /// Use the last actual input event, including pointer-only copy operations.
    /// Tracking event order also handles wrapping Wayland serial numbers.
    pub fn input_serial(&self) -> u32 {
        self.input_serial
    }

    /// Returns the latest tracked serial of the provided [`SerialKind`]
    ///
    /// Will return 0 if not tracked.
    pub fn get(&self, kind: SerialKind) -> u32 {
        self.serials
            .get(&kind)
            .map(|serial_data| serial_data.serial)
            .unwrap_or(0)
    }
}
