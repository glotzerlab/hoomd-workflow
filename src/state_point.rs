use serde::{Deserialize, Serialize};

/// Model parameters that describe a single simulation.
#[derive(Serialize, Deserialize)]
pub struct StatePoint {
    pub n: usize,
    pub epsilon: f64,
    pub sigma: f64,
    pub temperature: f64,
    pub number_density: f64,
    pub replicate: u32,
}
