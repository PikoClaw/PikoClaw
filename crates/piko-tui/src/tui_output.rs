use crate::events::AppEvent;
use async_trait::async_trait;
use piko_agent::output::{AgentEvent, OutputSink};
use tokio::sync::mpsc;

pub struct TuiOutputSink {
    tx: mpsc::UnboundedSender<AppEvent>,
}

impl TuiOutputSink {
    pub fn new(tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl OutputSink for TuiOutputSink {
    async fn emit(&self, event: AgentEvent) {
        let _ = self.tx.send(AppEvent::Agent(event));
    }
}
