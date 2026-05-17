use std::path::{Path, PathBuf};
use std::process::Stdio;
use anyhow::{Context, Result};
use nats_common::Action;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, error, info};

/// Scans the provisioning directory hierarchy for matching python scripts and executes them.
///
/// Hierarchy levels (from least to most specific):
/// 1. `{root}/{domain_slug}/`
/// 2. `{root}/{domain_slug}/{hw_version}/`
/// 3. `{root}/{domain_slug}/{hw_version}/{sw_version}/`
/// 4. `{root}/{domain_slug}/{hw_version}/{sw_version}/{serial_number}/`
///
/// In each directory, it looks for scripts named:
/// - `{event_type}_add.py`
/// - `{event_type}_update.py`
/// - `{event_type}_delete.py`
pub async fn run_scripts(
    root_dir: &Path,
    domain_slug: &str,
    hw_version: Option<&str>,
    sw_version: Option<&str>,
    serial_number: &str,
    event_type: &str,
    payload_bytes: &[u8],
) -> Result<Vec<Action>> {
    let mut collected_actions = Vec::new();
    
    // Build the directories to scan, from general to specific
    let mut dirs_to_scan = Vec::new();
    
    let mut current_dir = root_dir.join(domain_slug);
    dirs_to_scan.push(current_dir.clone());
    
    if let Some(hw) = hw_version {
        current_dir = current_dir.join(hw);
        dirs_to_scan.push(current_dir.clone());
        
        if let Some(sw) = sw_version {
            current_dir = current_dir.join(sw);
            dirs_to_scan.push(current_dir.clone());
            
            current_dir = current_dir.join(serial_number);
            dirs_to_scan.push(current_dir);
        }
    }

    for dir in dirs_to_scan {
        if !dir.exists() || !dir.is_dir() {
            continue;
        }

        let script_names = [
            format!("{}_add.py", event_type),
            format!("{}_update.py", event_type),
            format!("{}_delete.py", event_type),
        ];

        for script_name in script_names {
            let script_path = dir.join(&script_name);
            if script_path.exists() && script_path.is_file() {
                debug!("Executing provisioning script: {:?}", script_path);
                match execute_python_script(&script_path, &payload_bytes).await {
                    Ok(mut actions) => {
                        info!("Script {:?} returned {} actions", script_path, actions.len());
                        collected_actions.append(&mut actions);
                    }
                    Err(e) => {
                        error!("Failed to execute script {:?}: {:#}", script_path, e);
                        // We continue executing other scripts even if one fails
                    }
                }
            }
        }
    }

    Ok(collected_actions)
}

async fn execute_python_script(script_path: &PathBuf, payload_bytes: &[u8]) -> Result<Vec<Action>> {
    let mut child = Command::new("python3")
        .arg(script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn python3 process")?;

    // Write payload to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload_bytes).await.context("Failed to write to script stdin")?;
    } else {
        anyhow::bail!("Failed to open stdin for python3 process");
    }

    let output = child.wait_with_output().await.context("Failed to wait on python3 process")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Script exited with status {}:\n{}", output.status, stderr);
    }

    if output.stdout.is_empty() {
        return Ok(Vec::new());
    }

    // Try to parse stdout as a JSON array of Actions
    let actions: Vec<Action> = serde_json::from_slice(&output.stdout)
        .context("Failed to parse script stdout as JSON array of Actions")?;

    Ok(actions)
}
