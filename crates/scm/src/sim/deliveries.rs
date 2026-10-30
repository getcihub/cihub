use std::ops::Range;

use super::{Sim, State, WebhookFaults};
use crate::{ScmDelivery, ScmEvent};

/// An attempt to send a delivery to CIHub.
#[derive(Clone, Debug)]
pub(super) struct Attempt {
    pub(super) delivery_id: String,
    pub(super) delivery: ScmDelivery,
    pub(super) is_delivered: bool,
}

impl Sim {
    /// The deliveries sent since the last take, in their order, then the
    /// attempts asked again. The `n`-th new delivery taken has the identifier
    /// `sim-n`; an attempt asked again keeps the identifier of its delivery.
    pub fn take_deliveries(&self) -> Vec<ScmDelivery> {
        self.with(|state| {
            let taken = state.take_attempts();
            state.attempts[taken]
                .iter()
                .map(|attempt| attempt.delivery.clone())
                .collect()
        })
    }

    /// The deliveries sent since the last take, lost, sent twice and
    /// reordered as `faults` says. A lost attempt is not delivered.
    pub fn take_deliveries_with(&self, faults: &mut WebhookFaults) -> Vec<ScmDelivery> {
        let mut deliveries = Vec::new();
        self.with(|state| {
            let taken = state.take_attempts();
            for attempt in &mut state.attempts[taken] {
                if faults.chance(faults.lose_percent) {
                    attempt.is_delivered = false;
                    continue;
                }

                if faults.chance(faults.duplicate_percent) {
                    deliveries.push(attempt.delivery.clone());
                }
                deliveries.push(attempt.delivery.clone());
            }
        });
        if faults.reorder {
            for i in (1..deliveries.len()).rev() {
                let j = usize::try_from(faults.next() % (i as u64 + 1)).unwrap_or(i);
                deliveries.swap(i, j);
            }
        }

        deliveries
    }
}

impl State {
    fn take_attempts(&mut self) -> Range<usize> {
        let first = self.attempts.len();
        for delivery in std::mem::take(&mut self.deliveries) {
            self.delivered += 1;
            self.attempts.push(Attempt {
                delivery_id: format!("sim-{}", self.delivered),
                delivery,
                is_delivered: true,
            });
        }

        for attempt in std::mem::take(&mut self.redelivered) {
            let again = Attempt {
                is_delivered: true,
                ..self.attempts[attempt].clone()
            };
            self.attempts.push(again);
        }

        first..self.attempts.len()
    }

    pub(super) fn connection_changed(&mut self, connection_external_id: &str) {
        self.deliveries.push(ScmDelivery {
            connection_external_id: Some(connection_external_id.to_owned()),
            event: ScmEvent::ConnectionChanged,
        });
    }
}
