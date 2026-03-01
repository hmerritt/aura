use std::time::Duration;
use tokio::time::{interval, Interval, MissedTickBehavior};

#[derive(Debug, Clone, Copy)]
pub enum SchedulerEvent {
    SwitchImage,
    RefreshRemote,
}

pub struct Scheduler {
    switch_interval: Interval,
    remote_interval: Interval,
}

impl Scheduler {
    pub fn new(switch_every: Duration, remote_every: Duration) -> Self {
        let mut switch_interval = interval(switch_every);
        switch_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut remote_interval = interval(remote_every);
        remote_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        Self {
            switch_interval,
            remote_interval,
        }
    }

    pub async fn next_event(&mut self) -> SchedulerEvent {
        tokio::select! {
            _ = self.switch_interval.tick() => SchedulerEvent::SwitchImage,
            _ = self.remote_interval.tick() => SchedulerEvent::RefreshRemote,
        }
    }
}
