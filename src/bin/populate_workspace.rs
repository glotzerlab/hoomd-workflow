use hoomd_workflow::StatePoint;

fn main() -> anyhow::Result<()> {
    hoomd_workspace::add(&StatePoint {
        n: 1_000,
        epsilon: 1.0,
        sigma: 1.0,
        temperature: 1.0,
        number_density: 0.4,
        replicate: 0,
    })?;

    hoomd_workspace::add(&StatePoint {
        n: 1_000,
        epsilon: 1.0,
        sigma: 1.0,
        temperature: 1.0,
        number_density: 0.4,
        replicate: 1,
    })?;

    hoomd_workspace::add(&StatePoint {
        n: 1_000,
        epsilon: 1.0,
        sigma: 1.0,
        temperature: 0.7,
        number_density: 0.4,
        replicate: 0,
    })?;

    Ok(())
}
