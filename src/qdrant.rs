use crate::config::Config;
use anyhow::{bail, Context, Result};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use std::process::Command;
use std::time::Duration;

/// Ensures Qdrant is running (via Docker) and the collection exists.
pub async fn ensure_qdrant(config: &Config) -> Result<Qdrant> {
    if !is_container_running(config) {
        start_container(config)?;
        wait_for_healthy(config).await?;
    }

    let client = connect(config).await?;
    ensure_collection(config, &client).await?;
    Ok(client)
}

/// Connect to a running Qdrant instance.
pub async fn connect(config: &Config) -> Result<Qdrant> {
    let url = format!("http://{}:{}", config.qdrant.host, config.qdrant.grpc_port);
    let client = Qdrant::from_url(&url)
        .build()
        .with_context(|| format!("failed to connect to Qdrant at {url}"))?;
    Ok(client)
}

fn is_container_running(config: &Config) -> bool {
    let name = &config.qdrant.docker_container_name;
    let output = Command::new("docker")
        .args([
            "ps",
            "--filter",
            &format!("name={}", name),
            "--format",
            "{{.Names}}",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.trim() == name.as_str()
        }
        Err(_) => false,
    }
}

fn start_container(config: &Config) -> Result<()> {
    let name = &config.qdrant.docker_container_name;
    let volume = &config.qdrant.docker_volume_name;
    let image = &config.qdrant.docker_image;
    let rest_port = config.qdrant.rest_port;
    let grpc_port = config.qdrant.grpc_port;

    // Check if container exists (stopped)
    let exists = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name={}", name),
            "--format",
            "{{.Names}}",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == name.as_str())
        .unwrap_or(false);

    if exists {
        println!("Starting existing container '{}'...", name);
        let status = Command::new("docker")
            .args(["start", name])
            .status()
            .context("failed to run docker start")?;
        if !status.success() {
            bail!("docker start failed for container '{}'", name);
        }
    } else {
        println!("Creating Qdrant container '{}'...", name);
        let status = Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                name.as_str(),
                "-v",
                &format!("{}:/qdrant/storage", volume),
                "-p",
                &format!("{}:6333", rest_port),
                "-p",
                &format!("{}:6334", grpc_port),
                image.as_str(),
            ])
            .status()
            .context("failed to run docker run")?;
        if !status.success() {
            bail!("docker run failed for image '{}'", image);
        }
    }

    Ok(())
}

async fn wait_for_healthy(config: &Config) -> Result<()> {
    let url = format!(
        "http://{}:{}/healthz",
        config.qdrant.host, config.qdrant.rest_port
    );
    let client = reqwest::Client::new();
    let max_attempts = 60;

    for i in 0..max_attempts {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                println!("Qdrant is healthy.");
                return Ok(());
            }
            _ => {
                if i == 0 {
                    print!("Waiting for Qdrant to become healthy...");
                }
                print!(".");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    bail!(
        "Qdrant did not become healthy within {} seconds",
        max_attempts / 2
    );
}

async fn ensure_collection(config: &Config, client: &Qdrant) -> Result<()> {
    let collection_name = &config.qdrant.collection_name;

    let collections = client.list_collections().await?;
    let exists = collections
        .collections
        .iter()
        .any(|c| c.name == *collection_name);

    if exists {
        println!("Collection '{}' already exists.", collection_name);
        return Ok(());
    }

    println!("Creating collection '{}'...", collection_name);

    let request = CreateCollectionBuilder::new(collection_name)
        .vectors_config(VectorParamsBuilder::new(
            config.embedding.dimension,
            Distance::Cosine,
        ));

    client.create_collection(request).await?;

    println!("Collection '{}' created.", collection_name);
    Ok(())
}

/// Delete the docker container and volume.
pub fn teardown(config: &Config) -> Result<()> {
    let name = &config.qdrant.docker_container_name;
    let volume = &config.qdrant.docker_volume_name;

    println!("Stopping and removing container '{}'...", name);
    Command::new("docker")
        .args(["rm", "-f", name.as_str()])
        .status()
        .context("failed to remove container")?;

    println!("Removing volume '{}'...", volume);
    Command::new("docker")
        .args(["volume", "rm", volume.as_str()])
        .status()
        .context("failed to remove volume")?;

    Ok(())
}
