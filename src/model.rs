use anyhow::{Context, anyhow};
use hoomd_geometry::{Volume, shape::Cuboid};
use hoomd_interaction::{
    MaximumInteractionRange, PairwiseCutoff,
    pairwise::Isotropic,
    univariate::{Expanded, LennardJones, OverlapPenalty},
};
use hoomd_mc::{Count, QuickCompress, QuickInsert, Sweep, Translate, Trial, Tune, UniformIn};
use hoomd_microstate::{Microstate, SiteKey, boundary::Periodic, property::Point};
use hoomd_simulation::{Simulation, macrostate::Isothermal};
use hoomd_spatial::VecCell;
use hoomd_vector::Cartesian;
use log::debug;
use serde::{Deserialize, Serialize};

use super::StatePoint;

const INITIAL_NUMBER_DENSITY: f64 = 0.2;
const INITIAL_MAXIMUM_DISTANCE: f64 = 0.1;
const RELAX_STEPS: u64 = 10_000;

type PositionVector = Cartesian<3>;
type BodyProperties = Point<PositionVector>;
type SiteProperties = Point<PositionVector>;
type Boundary = Periodic<Cuboid>;

#[derive(Serialize, Deserialize)]
pub enum Phase {
    Initialize,
    Relax,
    Equilibrate,
}

/// The Lennard-Jones simulation model.
#[derive(Serialize, Deserialize)]
pub struct LennardJonesModel {
    pub microstate: Microstate<BodyProperties, SiteProperties, VecCell<SiteKey, 3>, Boundary>,
    pub translate_sweep: Sweep<Translate<PositionVector>>,
    pub hamiltonian: PairwiseCutoff<Isotropic<LennardJones>>,
    pub quick_insert: QuickInsert<UniformIn<SiteProperties, Boundary>>,
    pub quick_compress: QuickCompress<Boundary>,
    pub overlap_penalty_hamiltonian: PairwiseCutoff<Isotropic<Expanded<OverlapPenalty>>>,
    pub macrostate: Isothermal,
    pub translate_count: Count,
    pub phase: Phase,
    pub relax_step: u64,
}

impl Simulation for LennardJonesModel {
    #[inline]
    fn advance(&mut self) -> anyhow::Result<()> {
        if self.microstate.step().is_multiple_of(300) {
            self.microstate.sort_sites();
        }

        match self.phase {
            Phase::Initialize => self.initialize().context("failed to initialize")?,
            Phase::Relax => self.relax(),
            Phase::Equilibrate => self.equilibrate(),
        }

        self.microstate.increment_step();

        Ok(())
    }

    #[inline]
    fn step(&self) -> u64 {
        self.microstate.step()
    }
}

impl LennardJonesModel {
    pub fn new(state_point: StatePoint) -> anyhow::Result<Self> {
        let macrostate = Isothermal {
            temperature: state_point.temperature,
        };

        let hamiltonian = PairwiseCutoff(Isotropic {
            interaction: LennardJones {
                epsilon: state_point.epsilon,
                sigma: state_point.sigma,
            },
            r_cut: 2.5 * state_point.sigma,
        });

        let initial_box_volume = state_point.n as f64 / INITIAL_NUMBER_DENSITY;
        let initial_box_edge_length = initial_box_volume.cbrt();
        let cuboid = Cuboid::with_equal_edges(initial_box_edge_length.try_into()?);
        let periodic_cuboid = Periodic::new(hamiltonian.maximum_interaction_range(), cuboid)?;

        let vec_cell = VecCell::builder()
            .nominal_search_radius(hamiltonian.maximum_interaction_range().try_into()?)
            .build();
        let microstate = Microstate::builder()
            .seed(state_point.replicate)
            .boundary(periodic_cuboid)
            .spatial_data(vec_cell)
            .try_build()?;

        let translate = Translate::with_maximum_distance(INITIAL_MAXIMUM_DISTANCE.try_into()?);
        let translate_sweep = Sweep(translate);

        let target_box_volume = state_point.n as f64 / state_point.number_density;
        let quick_compress = QuickCompress::with_target_volume(target_box_volume.try_into()?);

        let distribution = UniformIn {
            boundary: microstate.boundary().clone(),
            template_sites: vec![SiteProperties::default()],
        };
        let quick_insert = QuickInsert::new(distribution, state_point.n);

        let overlap_penalty = Isotropic {
            interaction: Expanded {
                delta: 1.0,
                f: OverlapPenalty::default(),
            },
            r_cut: 1.0,
        };

        let overlap_penalty_hamiltonian = PairwiseCutoff(overlap_penalty);

        Ok(LennardJonesModel {
            microstate,
            overlap_penalty_hamiltonian,
            hamiltonian,
            translate_sweep,
            quick_compress,
            quick_insert,
            macrostate,
            phase: Phase::Initialize,
            translate_count: Count::default(),
            relax_step: 0,
        })
    }

    fn initialize(&mut self) -> anyhow::Result<()> {
        if self.quick_insert.is_complete() {
            self.quick_compress.apply(
                &mut self.microstate,
                &self.overlap_penalty_hamiltonian,
                |_| true,
            );
        } else {
            self.quick_insert
                .apply(&mut self.microstate, &self.overlap_penalty_hamiltonian);
        }

        self.translate_count += self.translate_sweep.apply(
            &mut self.microstate,
            &self.overlap_penalty_hamiltonian,
            &Isothermal { temperature: 1.0 },
        );

        if self.quick_compress.is_complete() {
            self.translate_sweep.tune_default(
                &self.microstate,
                &self.hamiltonian,
                &self.macrostate,
            );

            self.phase = Phase::Relax;
            debug!(
                "Initialization complete at step {}.",
                self.microstate.step()
            );
        }

        if self.step() >= 20_000 {
            let n = self.microstate.bodies().len();
            let target_n = self.quick_insert.target();
            let volume = self.microstate.boundary().volume();
            let target_volume = self.quick_compress.target_volume();
            return Err(anyhow!(
                "inserted {n}/{target_n} bodies and compressed to {volume} / {target_volume}"
            ));
        }

        Ok(())
    }

    fn relax(&mut self) {
        self.translate_count +=
            self.translate_sweep
                .apply(&mut self.microstate, &self.hamiltonian, &self.macrostate);

        self.relax_step += 1;
        if self.relax_step == RELAX_STEPS {
            debug!(
                "Relax phase complete at step {}, retuning trial moves.",
                self.microstate.step()
            );
            self.translate_sweep.tune_default(
                &self.microstate,
                &self.hamiltonian,
                &self.macrostate,
            );

            self.phase = Phase::Equilibrate;
        }
    }

    fn equilibrate(&mut self) {
        self.translate_count +=
            self.translate_sweep
                .apply(&mut self.microstate, &self.hamiltonian, &self.macrostate);
    }

    pub fn clear_move_counts(&mut self) {
        self.translate_count = Count::default()
    }
}
