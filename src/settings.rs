pub struct Settings {
    pub pingpong_scroll: bool,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            pingpong_scroll: true,
        }
    }

    pub fn count(&self) -> usize {
        1
    }

    fn label(&self, idx: usize) -> Option<&'static str> {
        match idx {
            0 => Some("Long text effect"),
            _ => None,
        }
    }

    fn value(&self, idx: usize) -> Option<&'static str> {
        match idx {
            0 => Some(if self.pingpong_scroll {
                "Ping-pong"
            } else {
                "Continuous scroll"
            }),
            _ => None,
        }
    }

    /// Cycle the value at `idx`.
    pub fn cycle(&mut self, idx: usize) {
        match idx {
            0 => self.pingpong_scroll = !self.pingpong_scroll,
            _ => {}
        }
    }

    /// Display labels, one per setting. Order matches `values()`.
    pub fn items(&self) -> Vec<String> {
        (0..self.count())
            .filter_map(|i| self.label(i).map(String::from))
            .collect()
    }

    /// Current value display strings, one per setting. Order matches `items()`.
    pub fn values(&self) -> Vec<String> {
        (0..self.count())
            .filter_map(|i| self.value(i).map(String::from))
            .collect()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}
