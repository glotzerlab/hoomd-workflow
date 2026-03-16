use std::{
    env::{self, set_current_dir},
    fs::{self, File},
    io::{self, Write},
    path::Path,
    time::Instant,
};

use anyhow::Context;
use hoomd_geometry::Volume;
use hoomd_gsd::hoomd::HoomdGsdFile;
use hoomd_interaction::TotalEnergy;
use hoomd_microstate::AppendMicrostate;
use hoomd_simulation::Simulation;
use hoomd_utility::data::ParquetLogger;
use log::{debug, info};
use parquet_derive::ParquetRecordWriter;

use crate::{LennardJonesModel, StatePoint};

const MODEL_FILE: &str = "model.postcard";
const TOTAL_STEPS: u64 = 100_000;
const GSD_WRITE_PERIOD: u64 = 10_000;
const LOG_WRITE_PERIOD: u64 = 1_000;
const WALL_TIME_BUFFER: f64 = 300.0;

fn get_model() -> anyhow::Result<LennardJonesModel> {
    match fs::read(MODEL_FILE) {
        Ok(bytes) => {
            debug!("Continuing simulation from `{MODEL_FILE}`.");

            postcard::from_bytes(&bytes).with_context(|| format!("could not read `{MODEL_FILE}`"))
        }
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => {
                debug!("Constructing a new simulation model.");
                let state_point_bytes = fs::read("signac_statepoint.json")
                    .context("unable to read `signac_statepoint.json`")?;
                let state_point: StatePoint = serde_json::from_slice(&state_point_bytes)
                    .context("could not parse signac_statepoint.json")?;
                let _ = HoomdGsdFile::create("trajectory.in-progress.gsd");
                LennardJonesModel::new(state_point)
            }
            _ => Err(error).with_context(|| format!("Could not read `{MODEL_FILE}`")),
        },
    }
}

#[derive(ParquetRecordWriter)]
struct LogRecord {
    wall_time: f64,
    potential_energy: f64,
    volume: f64,
    translate_acceptance: f64,
}

pub fn simulate_one(directory: &Path) -> anyhow::Result<()> {
    let start = Instant::now();
    set_current_dir(directory)
        .with_context(|| format!("error switching to job directory `{}`", directory.display()))?;

    let mut model = get_model()
        .with_context(|| format!("error initializing model in `{}`", directory.display()))?;
    let mut gsd_file = HoomdGsdFile::open("trajectory.in-progress.gsd")
        .context("error opening trajectory.in-progress.gsd")?;
    let mut parquet_logger = ParquetLogger::<LogRecord>::create_unique("log.parquet")?;
    let maybe_wall_time_limit = match env::var("ACTION_WALLTIME_IN_MINUTES") {
        Ok(value) => {
            let parsed_value = value
                .parse::<f64>()
                .context("error parsing ACTION_WALLTIME_IN_MINUTES")?;
            debug!("Limiting wall time to {parsed_value} minutes.");
            Some(parsed_value * 60.0)
        }
        Err(_) => None,
    };

    while model.step() < TOTAL_STEPS {
        if model.step().is_multiple_of(1_000) {
            info!(
                "Step {} / {TOTAL_STEPS} ({:.0}%)",
                model.step(),
                model.step() as f64 / TOTAL_STEPS as f64 * 100.0
            );
        }
        model.advance()?;

        if model.step().is_multiple_of(GSD_WRITE_PERIOD) {
            gsd_file
                .append_microstate(&model.microstate)
                .context("error writing frame to trajectory.in-progress.gsd")?
                .end()?;
        }

        let wall_time = start.elapsed().as_secs_f64();
        if model.step().is_multiple_of(LOG_WRITE_PERIOD) {
            let log_record = LogRecord {
                wall_time,
                potential_energy: model.hamiltonian.total_energy(&model.microstate),
                volume: model.microstate.boundary().volume(),
                translate_acceptance: model.translate_count.acceptance_ratio().unwrap_or(0.0),
            };

            parquet_logger
                .log(log_record)
                .context("error writing to log.parquet")?;
        }

        if let Some(wall_time_limit) = maybe_wall_time_limit
            && wall_time + WALL_TIME_BUFFER > wall_time_limit
        {
            info!("Stopping simulation, wall time limit reached.");
            break;
        }
    }

    let out_bytes: Vec<u8> = postcard::to_stdvec(&model)?;
    let mut file = File::create(MODEL_FILE).context("failed to create `{MODEL_FILE}`")?;
    file.write_all(&out_bytes)
        .context("failed to write `{MODEL_FILE}`")?;

    drop(gsd_file);

    if model.step() == TOTAL_STEPS {
        info!("Simulation complete.");
        fs::rename("trajectory.in-progress.gsd", "trajectory.gsd")
            .context("failed to rename `trajectory.in-progress.gsd` to `trajectory.gsd`")?;
    }

    Ok(())
}
