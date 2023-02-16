use chrono::Utc;
use std::{collections::HashSet, time::Duration};
use tokio::{
    sync::mpsc::Receiver,
    time::{self, Instant},
};
use tokio_tasker::Stopper;
use tracing::{debug, info};

use crate::state::{AppState, Work};

pub async fn update_loop(_app_state: AppState, stopper: Stopper, rx: &mut Receiver<Work>) {
    let sleep = time::sleep(Duration::from_secs(1));
    tokio::pin!(sleep);

    let mut work: HashSet<Work> = HashSet::new();

    info!("update worker starting");

    while !stopper.is_stopped() {
        tokio::select! {
            biased;
            r = rx.recv() => {
                let val = r.unwrap();
                debug!("update worker got value: {:?}", val);
                work.retain(|k| k.namespace != val.namespace && k.deployment != val.deployment);
                work.insert(val);
            }
            () = &mut sleep => {
                sleep.as_mut().reset(Instant::now() + Duration::from_secs(1));
                debug!("update worker timed out, resetting sleep");
            }
        }

        if stopper.is_stopped() {
            break;
        }

        let now = Utc::now();

        let mut drained: Vec<Work> = vec![];
        for v in work.iter() {
            if v.occurred < now - chrono::Duration::seconds(10) {
                drained.push(v.clone());
            }
        }

        if stopper.is_stopped() {
            break;
        }

        for element in drained {
            debug!("update worker processing {:?}", element);
            work.remove(&element);
        }
    }
    info!("update worker stopped");
}