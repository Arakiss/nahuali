//! Time: one animation clock, a frame scheduler with a ceiling, and the
//! mascot animator. Nothing here reads the system clock; callers pass
//! `Instant`s, so every rule is unit-tested without sleeping.

use std::time::{Duration, Instant};

use nahuali_view::MascotState;

/// Decorative motion preset (`NAHUALI_MOTION=full|reduced|none`; `NO_MOTION`
/// set to anything means `none`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    Full,
    Reduced,
    None,
}

impl Motion {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "full" => Some(Self::Full),
            "reduced" => Some(Self::Reduced),
            "none" | "off" => Some(Self::None),
            _ => None,
        }
    }

    pub fn from_env() -> Self {
        if std::env::var_os("NO_MOTION").is_some() {
            return Self::None;
        }
        std::env::var("NAHUALI_MOTION").ok().and_then(|v| Self::parse(&v)).unwrap_or_default()
    }

    /// Shortest time a mascot frame may stay on screen.
    pub fn frame_floor(self) -> Duration {
        match self {
            Self::Full => Duration::from_millis(125),
            Self::Reduced | Self::None => Duration::from_millis(250),
        }
    }
}

/// The single clock everything that moves is aligned to: deadlines land on
/// multiples of a period from one epoch, so N animations cost one wake-up.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    epoch: Instant,
}

impl Clock {
    pub fn new(epoch: Instant) -> Self {
        Self { epoch }
    }

    /// The first grid point of `period` at or after `t`.
    pub fn align(&self, t: Instant, period: Duration) -> Instant {
        let period_ns = period.as_nanos().max(1);
        let since = t.saturating_duration_since(self.epoch).as_nanos();
        let steps = since.div_ceil(period_ns);
        let offset = steps * period_ns;
        self.epoch + Duration::from_nanos(u64::try_from(offset).unwrap_or(u64::MAX))
    }
}

/// Why a redraw was requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    Animation,
    Input,
}

/// Event-driven redraws with a ceiling. A request marks the screen dirty;
/// [`FrameScheduler::due`] says when the draw may happen: never sooner than
/// one frame after the last draw. Input wins over animation: pending input
/// is drained before drawing, and an input frame is never pushed back by
/// animation.
#[derive(Clone, Debug)]
pub struct FrameScheduler {
    min_interval: Duration,
    last: Option<Instant>,
    pending: Option<Reason>,
}

impl FrameScheduler {
    /// `fps` is the ceiling (30 by default).
    pub fn new(fps: u32) -> Self {
        Self { min_interval: Duration::from_secs(1) / fps.clamp(1, 120), last: None, pending: None }
    }

    pub fn request(&mut self, reason: Reason) {
        self.pending = Some(self.pending.map_or(reason, |p| p.max(reason)));
    }

    pub fn pending(&self) -> Option<Reason> {
        self.pending
    }

    /// When the next draw may run, if one is pending.
    pub fn due(&self, now: Instant) -> Option<Instant> {
        self.pending?;
        Some(match self.last {
            Some(last) => (last + self.min_interval).max(now),
            None => now,
        })
    }

    pub fn should_draw(&self, now: Instant) -> bool {
        self.due(now).is_some_and(|due| due <= now)
    }

    pub fn drawn(&mut self, now: Instant) {
        self.last = Some(now);
        self.pending = None;
    }
}

/// Minimum time a state stays visible, so "listening" and "thinking" never
/// flicker between each other.
pub const MIN_DWELL: Duration = Duration::from_millis(1500);

/// One frame's timing, as the atlas describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    pub ms: u32,
    pub jitter_ms: u32,
}

/// Which state and frame the axolotl shows, and when that changes.
///
/// - A requested state replaces the shown one only after [`MIN_DWELL`];
///   `Alarmed` is the one exception and preempts at once.
/// - Frames advance on the shared clock at no more than 8 fps (4 fps under
///   reduced motion or a protocol that retransmits every frame).
/// - Losing terminal focus freezes the pose; state changes still apply.
/// - Motion `none` shows the first frame of each state and never ticks.
#[derive(Clone, Debug)]
pub struct MascotAnimator {
    clock: Clock,
    motion: Motion,
    floor: Duration,
    shown: MascotState,
    shown_since: Instant,
    target: MascotState,
    frame: usize,
    frame_until: Option<Instant>,
    focused: bool,
    rng: u64,
}

impl MascotAnimator {
    pub fn new(clock: Clock, motion: Motion, initial: MascotState, now: Instant) -> Self {
        Self {
            clock,
            motion,
            floor: motion.frame_floor(),
            shown: initial,
            shown_since: now,
            target: initial,
            frame: 0,
            frame_until: None,
            focused: true,
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// Raise the frame floor (e.g. 250 ms for iTerm2, which resends frames).
    pub fn set_floor(&mut self, floor: Duration) {
        self.floor = floor.max(self.motion.frame_floor());
    }

    pub fn shown(&self) -> MascotState {
        self.shown
    }

    pub fn frame(&self) -> usize {
        self.frame
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    fn random(&mut self, bound: u32) -> u32 {
        // xorshift64*: deterministic, good enough for blink jitter.
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let value = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        if bound == 0 { 0 } else { (value % u64::from(bound + 1)) as u32 }
    }

    fn schedule(&mut self, now: Instant, timings: &[Timing]) {
        if self.motion == Motion::None || !self.focused || timings.len() < 2 {
            self.frame_until = None;
            return;
        }
        let timing = timings[self.frame % timings.len()];
        let hold = Duration::from_millis(u64::from(timing.ms + self.random(timing.jitter_ms)));
        let hold = hold.max(self.floor);
        self.frame_until = Some(self.clock.align(now + hold, self.floor));
    }

    /// Ask for `state`. Returns true if what is shown changed now.
    pub fn request(&mut self, state: MascotState, now: Instant, timings: &[Timing]) -> bool {
        self.target = state;
        self.try_switch(now, timings)
    }

    fn try_switch(&mut self, now: Instant, timings: &[Timing]) -> bool {
        if self.target == self.shown {
            if self.frame_until.is_none() {
                self.schedule(now, timings);
            }
            return false;
        }
        let urgent = self.target == MascotState::Alarmed;
        if !urgent && now.saturating_duration_since(self.shown_since) < MIN_DWELL {
            return false;
        }
        self.shown = self.target;
        self.shown_since = now;
        self.frame = 0;
        self.frame_until = None;
        true
    }

    /// Advance time. `timings` are the frames of the state currently shown
    /// (after any switch this call makes, call again with the new state's
    /// timings via [`MascotAnimator::retime`]). Returns true if the visible
    /// frame or state changed.
    pub fn tick(&mut self, now: Instant, timings: &[Timing]) -> bool {
        if self.try_switch(now, timings) {
            return true;
        }
        let Some(until) = self.frame_until else {
            self.schedule(now, timings);
            return false;
        };
        if now < until {
            return false;
        }
        self.frame = (self.frame + 1) % timings.len().max(1);
        self.schedule(now, timings);
        true
    }

    /// Re-plan the current frame after a state switch.
    pub fn retime(&mut self, now: Instant, timings: &[Timing]) {
        self.frame_until = None;
        self.schedule(now, timings);
    }

    /// Terminal focus changed. Freezing keeps the current pose.
    pub fn set_focus(&mut self, focused: bool, now: Instant, timings: &[Timing]) {
        self.focused = focused;
        if focused {
            self.schedule(now, timings);
        } else {
            self.frame_until = None;
        }
    }

    /// The next instant at which [`MascotAnimator::tick`] could change
    /// something, or `None` when nothing will move on its own.
    pub fn next_deadline(&self) -> Option<Instant> {
        let switch = (self.target != self.shown).then_some(self.shown_since + MIN_DWELL);
        match (self.frame_until, switch) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOP: [Timing; 3] = [
        Timing { ms: 600, jitter_ms: 0 },
        Timing { ms: 30, jitter_ms: 0 },
        Timing { ms: 400, jitter_ms: 400 },
    ];

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn clock_aligns_to_one_grid() {
        let t0 = Instant::now();
        let clock = Clock::new(t0);
        assert_eq!(clock.align(t0 + ms(1), ms(125)), t0 + ms(125));
        assert_eq!(clock.align(t0 + ms(250), ms(125)), t0 + ms(250));
        assert_eq!(clock.align(t0, ms(125)), t0);
    }

    #[test]
    fn scheduler_caps_at_thirty_fps_and_input_wins() {
        let t0 = Instant::now();
        let mut s = FrameScheduler::new(30);
        assert_eq!(s.due(t0), None);
        s.request(Reason::Animation);
        assert!(s.should_draw(t0));
        s.drawn(t0);
        s.request(Reason::Animation);
        s.request(Reason::Input);
        assert_eq!(s.pending(), Some(Reason::Input));
        assert!(!s.should_draw(t0 + ms(10)));
        let due = s.due(t0 + ms(10)).unwrap();
        assert!(due >= t0 + ms(33) && due <= t0 + ms(34));
        assert!(s.should_draw(t0 + ms(34)));
        s.drawn(t0 + ms(34));
        assert_eq!(s.pending(), None);
    }

    #[test]
    fn frames_never_exceed_eight_fps() {
        let t0 = Instant::now();
        let mut a = MascotAnimator::new(Clock::new(t0), Motion::Full, MascotState::Idle, t0);
        a.retime(t0, &LOOP);
        let mut t = t0;
        let mut changes = Vec::new();
        for _ in 0..2000 {
            t += ms(5);
            if a.tick(t, &LOOP) {
                changes.push(t);
            }
        }
        assert!(changes.len() > 5);
        for pair in changes.windows(2) {
            assert!(pair[1] - pair[0] >= ms(125), "{:?}", pair[1] - pair[0]);
        }
    }

    #[test]
    fn states_dwell_at_least_one_and_a_half_seconds() {
        let t0 = Instant::now();
        let mut a = MascotAnimator::new(Clock::new(t0), Motion::Full, MascotState::Idle, t0);
        assert!(a.request(MascotState::Thinking, t0 + ms(2000), &LOOP));
        assert!(!a.request(MascotState::Listening, t0 + ms(2500), &LOOP));
        assert_eq!(a.shown(), MascotState::Thinking);
        assert_eq!(a.next_deadline().unwrap().min(t0 + ms(3500)), t0 + ms(3500));
        assert!(a.tick(t0 + ms(3500), &LOOP));
        assert_eq!(a.shown(), MascotState::Listening);
    }

    #[test]
    fn alarm_preempts_the_dwell() {
        let t0 = Instant::now();
        let mut a = MascotAnimator::new(Clock::new(t0), Motion::Full, MascotState::Idle, t0);
        assert!(a.request(MascotState::Alarmed, t0 + ms(10), &LOOP));
        assert_eq!(a.shown(), MascotState::Alarmed);
    }

    #[test]
    fn losing_focus_freezes_the_pose() {
        let t0 = Instant::now();
        let mut a = MascotAnimator::new(Clock::new(t0), Motion::Full, MascotState::Idle, t0);
        a.retime(t0, &LOOP);
        a.set_focus(false, t0 + ms(10), &LOOP);
        assert_eq!(a.next_deadline(), None);
        assert!(!a.tick(t0 + ms(5000), &LOOP));
        assert_eq!(a.frame(), 0);
        a.set_focus(true, t0 + ms(5000), &LOOP);
        assert!(a.next_deadline().is_some());
    }

    #[test]
    fn no_motion_never_ticks() {
        let t0 = Instant::now();
        let mut a = MascotAnimator::new(Clock::new(t0), Motion::None, MascotState::Idle, t0);
        a.retime(t0, &LOOP);
        assert_eq!(a.next_deadline(), None);
        assert!(!a.tick(t0 + ms(10_000), &LOOP));
        // State changes still show, as a still pose.
        assert!(a.request(MascotState::Worried, t0 + ms(10_000), &LOOP));
        assert_eq!(a.frame(), 0);
    }

    #[test]
    fn reduced_motion_halves_the_ceiling() {
        let t0 = Instant::now();
        let mut a = MascotAnimator::new(Clock::new(t0), Motion::Reduced, MascotState::Idle, t0);
        a.retime(t0, &LOOP);
        let mut t = t0;
        let mut last = None;
        for _ in 0..2000 {
            t += ms(5);
            if a.tick(t, &LOOP) {
                if let Some(prev) = last {
                    assert!(t - prev >= ms(250));
                }
                last = Some(t);
            }
        }
    }

    #[test]
    fn motion_parses_presets() {
        assert_eq!(Motion::parse("reduced"), Some(Motion::Reduced));
        assert_eq!(Motion::parse("off"), Some(Motion::None));
        assert_eq!(Motion::parse("wild"), None);
    }
}
