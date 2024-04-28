use std::process::Command;

pub async fn remove_containers() {
    let remove_command = "docker rm -f $(docker ps -a -q)";
    let output = Command::new("sh")
        .arg("-c")
        .arg(remove_command)
        .output()
        .expect("Failed to execute command");

    println!("Output: {}", String::from_utf8_lossy(&output.stdout));
}
