use crate::chunker::Chunk;
use crate::config::Config;
use anyhow::{bail, Context, Result};
use qdrant_client::config::QdrantConfig;
use qdrant_client::qdrant::{
    point_id, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointId,
    PointStruct, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
    let client = QdrantConfig::from_url(&url)
        .skip_compatibility_check()
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

/// Upsert HyPE-generated vectors with their chunk payloads.
/// Each chunk produces multiple vectors (one per hypothetical question).
pub async fn upsert_chunks(
    client: &Qdrant,
    config: &Config,
    points: Vec<(Vec<f32>, Chunk)>,
) -> Result<()> {
    let collection_name = &config.qdrant.collection_name;
    let mut point_structs = Vec::with_capacity(points.len());

    for (i, (vector, chunk)) in points.iter().enumerate() {
        let mut hasher = DefaultHasher::new();
        chunk.chunk_id.hash(&mut hasher);
        i.hash(&mut hasher);
        let point_id = PointId {
            point_id_options: Some(point_id::PointIdOptions::Num(hasher.finish())),
        };

        let payload: std::collections::HashMap<String, qdrant_client::qdrant::Value> = [
            ("chunk_id".to_string(), chunk.chunk_id.clone().into()),
            ("note_path".to_string(), chunk.note_path.clone().into()),
            ("note_title".to_string(), chunk.note_title.clone().into()),
            ("file_type".to_string(), chunk.file_type.clone().into()),
            ("chunk_index".to_string(), (chunk.chunk_index as f64).into()),
            (
                "total_chunks_in_section".to_string(),
                (chunk.total_chunks_in_section as f64).into(),
            ),
        ]
        .into_iter()
        .collect();

        let mut payload = payload;
        if let Some(section) = &chunk.section {
            payload.insert("section".to_string(), section.clone().into());
        }
        if !chunk.tags.is_empty() {
            payload.insert(
                "tags".to_string(),
                qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::ListValue(
                        qdrant_client::qdrant::ListValue {
                            values: chunk
                                .tags
                                .iter()
                                .map(|t| qdrant_client::qdrant::Value {
                                    kind: Some(
                                        qdrant_client::qdrant::value::Kind::StringValue(
                                            t.clone(),
                                        ),
                                    ),
                                })
                                .collect(),
                        },
                    )),
                },
            );
        }

        point_structs.push(PointStruct::new(point_id, vector.clone(), payload));
    }

    client
        .upsert_points(UpsertPointsBuilder::new(collection_name, point_structs))
        .await
        .context("failed to upsert points")?;

    Ok(())
}

/// Delete all points from the collection.
pub async fn clear_collection(config: &Config, client: &Qdrant) -> Result<()> {
    let collection_name = &config.qdrant.collection_name;
    client
        .delete_points(
            DeletePointsBuilder::new(collection_name)
                .points(Filter::default()),
        )
        .await
        .with_context(|| format!("failed to clear collection '{}'", collection_name))?;
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
