use std::time::{Duration, Instant};

/// A simple rate limiter.
///
/// This rate limiter is useful for limiting the rate at
/// which an action can be performed.
///
/// Note that, regardless of `period`, the first action
/// will always be allowed.
///
/// # Examples
///
/// ```rust
/// use std::{time::Duration, thread::sleep};
/// use ira::extra::ratelimit::Ratelimit;
///
/// let mut ratelimit = Ratelimit::new(Duration::from_secs(1));
/// let mut actions = 0;
///
/// for _ in 0..3 {
///   if ratelimit.check() {
///     actions += 1;
///   }
///
///   sleep(Duration::from_millis(600));
/// }
///
/// assert_eq!(actions, 2);
/// ```
#[derive(Debug)]
pub struct Ratelimit {
	last: Option<Instant>,
	period: Duration,
}

impl Ratelimit {
	/// Creates a new rate limiter with the given period.
	#[must_use]
	pub fn new(period: Duration) -> Self {
		Self { last: None, period }
	}

	/// Checks if the rate limiter allows an action to be performed.
	///
	/// If the rate limiter allows the action, the rate limiter
	/// will be updated and the function will return `true`.
	///
	/// If the rate limiter does not allow the action, the function
	/// will return `false`.
	pub fn check(&mut self) -> bool {
		if let Some(last) = self.last {
			if last.elapsed() < self.period {
				return false;
			}
		}

		self.last = Some(Instant::now());
		true
	}
}
