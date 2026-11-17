//! A plain pomodoro for the menu bar: 25 minutes of focus, then a 5 minute break, and every fourth a 15 minute one.
//! A break starts by itself when a focus ends; the next focus waits for the owner.

pub const FOCUS: i64 = 25 * 60;
pub const SHORT_BREAK: i64 = 5 * 60;
pub const LONG_BREAK: i64 = 15 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn seconds(self) -> i64 {
        match self {
            Phase::Focus => FOCUS,
            Phase::ShortBreak => SHORT_BREAK,
            Phase::LongBreak => LONG_BREAK,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Pomodoro {
    phase: Option<Phase>,
    /// When the phase ends, while it runs.
    ends: i64,
    /// Seconds left, while paused.
    left: Option<i64>,
    /// Focus sessions finished since Binder started.
    done: u32,
}

impl Pomodoro {
    pub fn phase(&self) -> Option<Phase> {
        self.phase
    }

    pub fn paused(&self) -> bool {
        self.left.is_some()
    }

    pub fn done(&self) -> u32 {
        self.done
    }

    fn start(&mut self, phase: Phase, now: i64) {
        self.phase = Some(phase);
        self.ends = now + phase.seconds();
        self.left = None;
    }

    pub fn focus(&mut self, now: i64) {
        self.start(Phase::Focus, now);
    }

    pub fn pause(&mut self, now: i64) {
        if self.phase.is_some() && self.left.is_none() {
            self.left = Some((self.ends - now).max(0));
        }
    }

    pub fn resume(&mut self, now: i64) {
        if let Some(left) = self.left.take() {
            self.ends = now + left;
        }
    }

    pub fn stop(&mut self) {
        self.phase = None;
        self.left = None;
    }

    fn left(&self, now: i64) -> Option<i64> {
        self.phase?;
        Some(self.left.unwrap_or((self.ends - now).max(0)))
    }

    /// `MM:SS` left, while a phase runs or is paused.
    pub fn label(&self, now: i64) -> Option<String> {
        let s = self.left(now)?;
        Some(format!("{:02}:{:02}", s / 60, s % 60))
    }

    /// The phase that just ended and the one that started after it, if any.
    pub fn tick(&mut self, now: i64) -> Option<(Phase, Option<Phase>)> {
        let phase = self.phase?;
        if self.left.is_some() || now < self.ends {
            return None;
        }
        if phase == Phase::Focus {
            self.done += 1;
            let next = if self.done % 4 == 0 { Phase::LongBreak } else { Phase::ShortBreak };
            self.start(next, now);
            Some((phase, Some(next)))
        } else {
            self.stop();
            Some((phase, None))
        }
    }
}

/// A finished focus session, for the count in the menu.
pub fn record(db: &mut crate::Db, at: i64, minutes: i64) -> crate::Result<()> {
    db.conn.execute("INSERT INTO pomodoros (at, minutes) VALUES (?1, ?2)", [at, minutes])?;
    Ok(())
}

/// Focus sessions finished since `since`.
pub fn count_since(db: &crate::Db, since: i64) -> crate::Result<i64> {
    Ok(db.conn.query_row("SELECT COUNT(*) FROM pomodoros WHERE at >= ?1", [since], |r| r.get(0))?)
}
