use std::path::{Path, PathBuf};
use std::process::Stdio;
use anyhow::{Context, Result};
use nats_common::Action;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, error, info};

/// Scans the provisioning directory hierarchy for matching python scripts and executes them.
///
/// Directory layout (event-type first, general → specific):
///
/// ```text
/// {root}/
/// └── {event_type}/               ← e.g. "inform", "session_ended"
///     └── {domain_slug}/          ← e.g. "default", "acme"
///         ├── add.py              ← new device
///         ├── update.py           ← known device
///         ├── delete.py           ← decommission (future internal event)
///         └── {hw_version}/
///             ├── add.py
///             └── {sw_version}/
///                 ├── add.py
///                 └── {device_id}/    ← "{oui}-{serial}", e.g. "AABB00-1234567"
///                     └── add.py
/// ```
///
/// All scripts that exist at each level are executed in order (general → specific).
/// Their returned actions are merged into a single list.
pub async fn run_scripts(
    root_dir: &Path,
    event_type: &str,
    domain_slug: &str,
    hw_version: Option<&str>,
    sw_version: Option<&str>,
    device_id: &str,
    payload_bytes: &[u8],
) -> Result<Vec<Action>> {
    let mut collected_actions = Vec::new();

    // Base: root / event_type / domain_slug
    let base = root_dir.join(event_type).join(domain_slug);

    // Build candidate directories from general to specific
    let mut dirs_to_scan: Vec<PathBuf> = vec![base.clone()];

    if let Some(hw) = hw_version {
        let hw_dir = base.join(hw);
        dirs_to_scan.push(hw_dir.clone());

        if let Some(sw) = sw_version {
            let sw_dir = hw_dir.join(sw);
            dirs_to_scan.push(sw_dir.clone());

            dirs_to_scan.push(sw_dir.join(device_id));
        }
    }

    // Script names are fixed regardless of event type
    const SCRIPT_NAMES: [&str; 3] = ["add.py", "update.py", "delete.py"];

    for dir in &dirs_to_scan {
        if !dir.is_dir() {
            continue;
        }

        for &script_name in &SCRIPT_NAMES {
            let script_path = dir.join(script_name);
            if !script_path.is_file() {
                continue;
            }

            debug!(script = ?script_path, "Executing provisioning script");

            match execute_python_script(&script_path, payload_bytes).await {
                Ok(mut actions) => {
                    info!(
                        script = ?script_path,
                        count  = actions.len(),
                        "Script returned actions",
                    );
                    collected_actions.append(&mut actions);
                }
                Err(e) => {
                    // Log and continue — a failing script must not abort the others.
                    error!(script = ?script_path, error = ?e, "Provisioning script failed");
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
