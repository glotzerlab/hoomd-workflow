use std::{env::set_current_dir, fs, io, path::Path};

use anyhow::Context;
use hoomd_simulation::Simulation;
use log::{debug, info};

use crate::{LennardJonesModel, StatePoint};

const MODEL_FILE: &str = "model.postcard";
const TOTAL_STEPS: u64 = 100_000;

fn get_model() -> anyhow::Result<LennardJonesModel> {
    match fs::read(MODEL_FILE) {
        Ok(bytes) => {
            debug!("Continuing simulation: '{MODEL_FILE}'.");

            postcard::from_bytes(&bytes)
                .with_context(|| format!("could not read {MODEL_FILE}"))
        }
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => {
                debug!("Constructing new Model");
                let state_point_bytes = fs::read("signac_statepoint.json").context("unable to read `signac_statepoint.json`")?;
                let state_point: StatePoint = serde_json::from_slice(&state_point_bytes).context("could not parse signac_statepoint.json")?;
                LennardJonesModel::new(state_point)
            },
            _ => return Err(error).with_context(|| format!("Could not read {MODEL_FILE}")),
        },
    }
}

// TODO: GSD and Log file

pub fn simulate_one(directory: &Path) -> anyhow::Result<()> {
    set_current_dir(directory).context("error switching to job directory")?;

    let mut model = get_model()?;

    while model.step() < TOTAL_STEPS {
        if model.step().is_multiple_of(1_000) {
            info!("Step {} / TOTAL_STEPS ({}%)", model.step(), model.step() as f64 /
            TOTAL_STEPS as f64);
        }
        model.advance()?;
    }

    Ok(())
}
