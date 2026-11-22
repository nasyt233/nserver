//! Docker CLI integration for listing and controlling containers/images.

use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: String,
}

#[derive(Debug, Clone, Default)]
pub struct DockerImage {
    pub repository: String,
    pub tag: String,
    pub id: String,
    pub size: String,
    pub created: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockerView {
    Containers,
    Images,
}

impl DockerView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Containers => "容器",
            Self::Images => "镜像",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DockerSnapshot {
    pub installed: bool,
    pub daemon_running: bool,
    pub version: Option<String>,
    pub containers: Vec<DockerContainer>,
    pub images: Vec<DockerImage>,
    pub error: Option<String>,
}

impl DockerSnapshot {
    pub fn refresh(&mut self) {
        self.error = None;
        match command("docker", &["version", "--format", "{{.Server.Version}}"]) {
            Ok(version) => {
                self.installed = true;
                self.daemon_running = true;
                self.version = Some(version.trim().to_string());
            }
            Err(err) => {
                self.installed = docker_binary_exists();
                self.daemon_running = false;
                self.version = None;
                self.error = Some(err);
                self.containers.clear();
                self.images.clear();
                return;
            }
        }
        match command(
            "docker",
            &[
                "ps",
                "-a",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Status}}\t{{.Ports}}",
            ],
        ) {
            Ok(text) => {
                self.containers = parse_containers(&text);
            }
            Err(err) => {
                self.error = Some(err);
            }
        }
        match command(
            "docker",
            &[
                "images",
                "--format",
                "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}\t{{.CreatedSince}}",
            ],
        ) {
            Ok(text) => {
                self.images = parse_images(&text);
            }
            Err(err) => {
                if self.error.is_none() {
                    self.error = Some(err);
                }
            }
        }
    }

    pub fn running_count(&self) -> usize {
        self.containers
            .iter()
            .filter(|c| c.state.eq_ignore_ascii_case("running"))
            .count()
    }
}

fn docker_binary_exists() -> bool {
    [
        "/usr/bin/docker",
        "/usr/local/bin/docker",
        "/snap/bin/docker",
    ]
    .iter()
    .any(|path| std::path::Path::new(path).is_file())
        || std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join("docker"))
                    .any(|path| path.is_file())
            })
            .unwrap_or(false)
}

pub fn action(id: &str, verb: &str) -> Result<String, String> {
    if !matches!(verb, "start" | "stop" | "restart" | "pause" | "unpause") {
        return Err("不支持的 Docker 操作。".into());
    }
    command("docker", &[verb, id])
}

pub fn logs(id: &str, lines: usize) -> Result<String, String> {
    command("docker", &["logs", "--tail", &lines.to_string(), id])
}

pub fn remove_container(id: &str) -> Result<String, String> {
    command("docker", &["rm", "-f", id])
}

pub fn remove_image(id: &str) -> Result<String, String> {
    command("docker", &["rmi", id])
}

fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .map_err(|err| format!("无法执行 docker：{err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        if stdout.trim().is_empty() {
            Ok("命令执行成功。".into())
        } else {
            Ok(stdout)
        }
    } else {
        Err(if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        })
    }
}

fn parse_containers(text: &str) -> Vec<DockerContainer> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 6 {
                return None;
            }
            Some(DockerContainer {
                id: fields[0].trim().to_string(),
                name: fields[1].trim().to_string(),
                image: fields[2].trim().to_string(),
                state: fields[3].trim().to_string(),
                status: fields[4].trim().to_string(),
                ports: fields[5].trim().to_string(),
            })
        })
        .collect()
}

fn parse_images(text: &str) -> Vec<DockerImage> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 5 {
                return None;
            }
            Some(DockerImage {
                repository: fields[0].trim().to_string(),
                tag: fields[1].trim().to_string(),
                id: fields[2].trim().to_string(),
                size: fields[3].trim().to_string(),
                created: fields[4].trim().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_container_rows() {
        let text = "abc123\tweb\tnginx:latest\trunning\tUp 2 hours\t0.0.0.0:80->80/tcp\n";
        let rows = parse_containers(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "web");
        assert_eq!(rows[0].state, "running");
    }

    #[test]
    fn parses_image_rows() {
        let text = "nginx\tlatest\tabcdef\t187MB\t2 weeks ago\n";
        let rows = parse_images(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].repository, "nginx");
    }

    #[test]
    fn unknown_verb_is_rejected() {
        assert!(action("x", "rm -rf").is_err());
    }
}
