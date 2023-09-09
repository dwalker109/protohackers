use crate::{
    codec::MsgFramed,
    message::{Action, Msg, TargetPopulation, VisitPopulation},
};
use std::collections::{hash_map::Entry, HashMap};
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct State {
    connections: HashMap<u32, MsgFramed>,
    targets: HashMap<u32, Vec<TargetPopulation>>,
    policies: HashMap<(u32, String), u32>,
}

impl State {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            targets: HashMap::new(),
            policies: HashMap::new(),
        }
    }

    pub async fn init_site(&mut self, site_id: &u32) {
        if let Entry::Vacant(entry) = self.connections.entry(*site_id) {
            let stream = TcpStream::connect("pestcontrol.protohackers.com:20547")
                .await
                .unwrap();

            let mut msg_framed = MsgFramed::new(stream).await;

            msg_framed.send(Msg::DialAuthority { site: *site_id }).await;

            let msg = msg_framed.next().await;
            let target_populations = match msg {
                Some(Ok(Msg::TargetPopulations { populations, .. })) => populations,
                _ => panic!("{msg:?}"),
            };

            entry.insert(msg_framed);
            self.targets.insert(*site_id, target_populations);
        }
    }

    pub async fn process_site(&mut self, site_id: &u32, visit_populations: &[VisitPopulation]) {
        let msg_framed = self.connections.get_mut(site_id).unwrap();

        for tp in self.targets.get(site_id).unwrap() {
            let count = visit_populations
                .iter()
                .find_map(|vp| (tp.species == vp.species).then_some(vp.count))
                .unwrap_or_default();

            let key = (*site_id, tp.species.clone());

            if let Some(policy) = self.policies.remove(&key) {
                msg_framed.send(Msg::DeletePolicy { policy }).await;

                let msg = msg_framed.next().await;
                if !matches!(msg, Some(Ok(Msg::Ok))) {
                    panic!("{msg:?}");
                }
            }

            let action = {
                if count < tp.min {
                    Some(Action::Conserve)
                } else if count > tp.max {
                    Some(Action::Cull)
                } else {
                    None
                }
            };

            if let Some(action) = action {
                msg_framed
                    .send(Msg::CreatePolicy {
                        species: tp.species.clone(),
                        action,
                    })
                    .await;

                let msg = msg_framed.next().await;
                match msg {
                    Some(Ok(Msg::PolicyResult { policy })) => {
                        self.policies.insert(key, policy);
                    }
                    _ => panic!("{msg:?}"),
                }
            }
        }
    }
}
