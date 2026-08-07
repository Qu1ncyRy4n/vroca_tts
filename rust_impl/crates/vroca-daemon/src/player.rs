use std::process::{Child, Command};
use vroca_core::operations::ProtocolError;

pub struct Player {
    child: Option<Child>,
}

impl Player {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn start_fake(&mut self) -> Result<(), ProtocolError> {
        let child = Command::new("sleep").arg("100").spawn().map_err(|e| {
            ProtocolError::invalid_request(format!("Failed to start fake player: {}", e))
        })?;
        self.child = Some(child);
        Ok(())
    }

    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,
                _ => false,
            }
        } else {
            false
        }
    }
}
