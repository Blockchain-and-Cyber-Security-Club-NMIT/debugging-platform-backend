use std::env;
use std::fs;
use std::io::{self, Read};
use std::path;
use std::process::{Command, Stdio};
use std::time::Duration;
use uuid::Uuid;
use wait_timeout::ChildExt; // Import the wait_timeout extension.

pub async fn execute_code(code: &str) -> io::Result<String> {
    let temp_dir = env::temp_dir();
    let uuid = Uuid::new_v4();
    let new_dir_path = temp_dir.join(uuid.to_string());
    fs::create_dir(&new_dir_path).expect("Failed to create directory");
    fs::write(new_dir_path.join(path::Path::new("Solution.java")), code).unwrap();
    let container_id_command = format!(
        "docker create -m 256m --memory-swap 256m -v {}:/data 860x9/java-executor",
        new_dir_path
            .to_str()
            .expect("Failed to convert path to string")
    );

    let container_id_output = Command::new("sh")
        .arg("-c")
        .arg(container_id_command)
        .output()
        .expect("Failed to execute command");

    let container_id = String::from_utf8_lossy(&container_id_output.stdout)
        .trim()
        .to_string();
    let execute_command = format!("docker start -a {}", container_id);

    // Start the Docker process
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(execute_command)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start process");

    let timeout = Duration::from_secs(1);
    let result = match child
        .wait_timeout(timeout)
        .expect("Failed to wait on child")
    {
        Some(status) if status.success() => {
            println!("Process exited successfully");
            let mut output = child.stdout.unwrap();
            let mut s = String::new();
            output
                .read_to_string(&mut s)
                .expect("Could not read stdout");

            Ok(s)
        }
        None =>
        // Ensure the process is cleaned up
        {
            Err(io::Error::new(io::ErrorKind::Other, "Process timed out"))
        }

        _ => {
            let stdout_path = new_dir_path.join(path::Path::new("stdout.log"));
            let stderr_path = new_dir_path.join(path::Path::new("stderr.log"));
            store_stdout(container_id.as_str(), stdout_path.to_str().unwrap());
            store_stderr(container_id.as_str(), stderr_path.to_str().unwrap());
            // Read stdout.log and stderr.log
            let stdout = std::fs::read_to_string(stdout_path).unwrap();
            assert_eq!(stdout, "");
            let stderr = std::fs::read_to_string(stderr_path).unwrap();
            println!("stdout: {}", stdout);
            println!("stderr: {}", stderr);
            Err(io::Error::new(io::ErrorKind::Other, format!("{}", stderr)))
        }
    };
    let rm_containers = "docker rm -f $(docker ps -a -q)";
    Command::new("sh")
        .arg("-c")
        .arg(rm_containers)
        .output()
        .expect("Failed to remove containers");
    result
}

fn store_stdout(container_id: &str, path: &str) -> String {
    let container_logs = format!("docker logs {} 1> {}", container_id, path);
    let output = Command::new("sh")
        .arg("-c")
        .arg(container_logs)
        .output()
        .expect("Failed to execute command");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn store_stderr(container_id: &str, path: &str) -> String {
    let container_logs = format!("docker logs {} 2> {}", container_id, path);
    let output = Command::new("sh")
        .arg("-c")
        .arg(container_logs)
        .output()
        .expect("Failed to execute command");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
