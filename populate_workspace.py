"""Populate the workspace."""

import signac

project = signac.get_project()

job = project.open_job(
    {
        "n": 1_000,
        "epsilon": 1.0,
        "sigma": 1.0,
        "temperature": 1.0,
        "number_density": 0.4,
        "replicate": 0,
    }
).init()

job = project.open_job(
    {
        "n": 1_000,
        "epsilon": 1.0,
        "sigma": 1.0,
        "temperature": 1.0,
        "number_density": 0.4,
        "replicate": 1,
    }
).init()

job = project.open_job(
    {
        "n": 1_000,
        "epsilon": 1.0,
        "sigma": 1.0,
        "temperature": 0.7,
        "number_density": 0.4,
        "replicate": 0,
    }
).init()
