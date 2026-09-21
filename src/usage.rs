use std::sync::{Arc, Mutex};

use flux::prelude::Id;

#[derive(Clone, Debug)]
pub enum UsageSource {
    Agent { agent_id: Id },
    Space { space_id: Id },
}

#[derive(Clone, Debug)]
pub struct UsageReport {
    pub source: UsageSource,
    pub provider: String,
    pub cost: f32,
    pub quantity: f32,
    pub unit: String,
}

#[derive(Clone, Debug, Default)]
pub struct UsageReporter {
    sender: Option<async_channel::Sender<UsageReport>>,
    budget: UsageBudget,
    source: Option<UsageSource>,
}

impl UsageReporter {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn new(
        sender: async_channel::Sender<UsageReport>,
        budget: UsageBudget,
        source: UsageSource,
    ) -> Self {
        Self {
            sender: Some(sender),
            budget,
            source: Some(source),
        }
    }

    pub fn is_available(&self) -> bool {
        self.sender.is_none() || self.budget.is_available()
    }

    pub fn report(
        &self,
        provider: impl Into<String>,
        cost: f32,
        quantity: f32,
        unit: impl Into<String>,
    ) {
        if cost <= 0.0 {
            return;
        }

        self.budget.record(cost);
        let Some(sender) = &self.sender else {
            return;
        };
        let Some(source) = &self.source else {
            return;
        };

        if let Err(error) = sender.try_send(UsageReport {
            source: source.clone(),
            provider: provider.into(),
            cost,
            quantity,
            unit: unit.into(),
        }) {
            tracing::warn!(?error, "Usage report was not accepted by the usage worker");
        }
    }

    pub fn budget(&self) -> UsageBudget {
        self.budget.clone()
    }

    pub fn same_budget(&self, other: &Self) -> bool {
        match (&self.sender, &other.sender) {
            (None, None) => true,
            (Some(_), Some(_)) => self.budget.same_budget(&other.budget),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UsageSnapshot {
    pub used: f32,
    pub limit: Option<f32>,
}

#[derive(Debug, Default)]
struct UsageBudgetState {
    used: f32,
    limit: Option<f32>,
}

/// Shared, in-memory admission gate for all workers billed to one owner.
#[derive(Clone, Debug, Default)]
pub struct UsageBudget {
    state: Arc<Mutex<UsageBudgetState>>,
}

impl UsageBudget {
    pub fn new(used: f32, limit: Option<f32>) -> Self {
        Self {
            state: Arc::new(Mutex::new(UsageBudgetState { used, limit })),
        }
    }

    pub fn is_available(&self) -> bool {
        let state = self.state.lock().expect("usage budget lock poisoned");
        state.limit.is_none_or(|limit| state.used < limit)
    }

    pub fn record(&self, cost: f32) {
        if cost <= 0.0 {
            return;
        }
        let mut state = self.state.lock().expect("usage budget lock poisoned");
        state.used += cost;
    }

    pub fn set_limit(&self, used: f32, limit: Option<f32>) {
        let mut state = self.state.lock().expect("usage budget lock poisoned");
        state.used = used;
        state.limit = limit;
    }

    pub fn snapshot(&self) -> UsageSnapshot {
        let state = self.state.lock().expect("usage budget lock poisoned");
        UsageSnapshot {
            used: state.used,
            limit: state.limit,
        }
    }

    fn same_budget(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}

#[cfg(test)]
mod tests {
    use super::UsageBudget;

    #[test]
    fn shared_budget_blocks_and_reopens() {
        let first = UsageBudget::new(0.0, Some(1.0));
        let second = first.clone();

        first.record(0.75);
        assert!(second.is_available());

        second.record(0.25);
        assert!(!first.is_available());

        first.set_limit(1.0, Some(2.0));
        assert!(second.is_available());
    }
}
