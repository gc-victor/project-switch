use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{command, Args, Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const PROJECT_SWITCH_SCRIPT_ZSH: &str = include_str!("./project-switch.zsh");
const PROJECT_SWITCH_SCRIPT_BASH: &str = include_str!("./project-switch.bash");

#[derive(Parser)]
#[command(name = "Project Switch")]
#[command(version = VERSION)]
#[command(about = "The CLI to switch between your projects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new project based on current path
    Add(AddArgs),
    /// Project directory path
    Dir(DirArgs),
    /// Switch to a project
    Go(GoArgs),
    /// Initialize the project switch by shell
    Init,
    /// Show a list of stored projects
    List(ListArgs),
    /// Remove the project
    Remove(RemoveArgs),
}

#[derive(Args)]
struct AddArgs {
    /// Name of the project
    name: Option<String>,
}

#[derive(Args)]
struct DirArgs {
    /// Name of the project
    name: String,
}

#[derive(Args)]
struct GoArgs {
    /// Name of the project
    name: String,
}

#[derive(Args)]
struct ListArgs {
    /// Show list of projects and paths in raw format
    #[arg(short, long, default_value = "false")]
    raw: bool,
}

#[derive(Args)]
struct RemoveArgs {
    /// Name of the project to remove
    name: String,
}

struct ProjectSwitch {
    ps_dir: PathBuf,
    projects_db: PathBuf,
}

impl ProjectSwitch {
    fn new() -> io::Result<Self> {
        let config_dir = dirs::config_dir().ok_or(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not find config directory",
        ))?;

        let is_test = env::var("IS_TEST").is_ok();
        let ps_dir = if is_test {
            config_dir.join("project-switch_test")
        } else {
            #[cfg(not(test))]
            #[allow(unused_variables)]
            let ps_dir = config_dir.join("project-switch");
            #[cfg(test)]
            let ps_dir = config_dir.join("project-switch_test");

            ps_dir
        };
        Ok(ProjectSwitch {
            projects_db: ps_dir.join("projects"),
            ps_dir,
        })
    }

    fn initialize(&self) -> io::Result<()> {
        fs::create_dir_all(&self.ps_dir)?;

        if !self.projects_db.exists() {
            let mut file = File::create(&self.projects_db)?;
            writeln!(file, "# File to store your projects for PS")?;
        }

        Ok(())
    }

    fn init(&self) -> io::Result<()> {
        let ps = ProjectSwitch::new()?;
        ps.initialize()?;

        let shell = shell()?;
        let shell = shell.as_str();

        let dir = match self.ps_dir.to_str() {
            Some(dir) => dir,
            None => {
                eprintln!("Failed to get the path of the project switch config directory");
                return Ok(());
            }
        };
        let script = match shell {
            "bash" => PROJECT_SWITCH_SCRIPT_BASH
                .to_string()
                .replace("__path__", dir),
            "zsh" => PROJECT_SWITCH_SCRIPT_ZSH
                .to_string()
                .replace("__path__", dir),
            _ => String::new(),
        };

        let script_path = ps.ps_dir.join("project_switch.sh");
        fs::write(&script_path, script)?;

        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }

        let profile = match shell {
            "bash" => "~/.bashrc",
            "zsh" => "~/.zshrc",
            _ => "~/.profile",
        };

        let profile = shellexpand::tilde(profile).to_string();
        let profile = Path::new(&profile);

        let mut file = OpenOptions::new().append(true).open(profile)?;
        let content = fs::read_to_string(profile)?;
        if content.contains(script_path.to_string_lossy().as_ref()) {
            eprintln!("Project Switch already initialized for {}", shell);
            return Ok(());
        }
        writeln!(file, "source {}", script_path.to_string_lossy())?;

        eprintln!("Project Switch initialized for {}", shell);

        Ok(())
    }

    fn add_project(&self, name: String) -> io::Result<()> {
        let current_dir = env::current_dir()?;
        let project_name = if !name.is_empty() {
            name
        } else {
            current_dir
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        };

        let project_path = current_dir.to_str().unwrap();

        if self.project_exists(&project_name)? {
            eprintln!("The project {} already exists", project_name);
            return Ok(());
        }

        let mut file = OpenOptions::new().append(true).open(&self.projects_db)?;
        writeln!(file, "{}:{}", project_name, project_path)?;
        eprintln!("Added project: {}", project_name);

        Ok(())
    }

    fn list_projects(&self, raw: bool) -> io::Result<()> {
        let file = File::open(&self.projects_db)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }

            let output = if raw {
                line.trim()
            } else {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    parts[0].trim()
                } else {
                    ""
                }
            };

            eprintln!("{}", output.trim());
        }

        Ok(())
    }

    fn remove_project(&self, name: &str) -> io::Result<()> {
        let content = fs::read_to_string(&self.projects_db)?;
        let mut new_content = String::new();
        let mut found = false;

        for line in content.lines() {
            if line.starts_with(&format!("{}:", name)) {
                found = true;
            } else {
                new_content.push_str(line);
                new_content.push('\n');
            }
        }

        if found {
            fs::write(&self.projects_db, new_content)?;
            eprintln!("Removed project: {}", name);
        } else {
            eprintln!("Project {} not found", name);
        }

        Ok(())
    }

    fn go_to_project(&self, name: String) -> io::Result<()> {
        let content = fs::read_to_string(&self.projects_db)?;
        for line in content.lines() {
            if line.starts_with(&format!("{}:", name)) {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let path = Path::new(parts[1]);
                    if path.exists() {
                        let shell = shell()?;
                        let shell = shell.as_str();

                        let script_path = self.ps_dir.join("project_switch.sh");
                        let script_path = script_path.to_str().unwrap();

                        let mut cmd = Command::new(shell);
                        cmd.arg("-c");
                        cmd.arg(format!("{} {}", script_path, name));

                        eprintln!("{} {}", script_path, name);

                        match cmd.status() {
                            Ok(_) => {
                                eprintln!("Switched to project: {}", name);
                            }

                            Err(e) => {
                                eprintln!("Failed to switch to project: {}", e);
                            }
                        }

                        return Ok(());
                    }
                }
            }
        }

        eprintln!("Project {} not found", name);
        Ok(())
    }

    fn project_dir(&self, name: String) -> io::Result<()> {
        let content = fs::read_to_string(&self.projects_db)?;
        for line in content.lines() {
            if line.starts_with(&format!("{}:", name)) {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let path = Path::new(parts[1]);
                    if path.exists() {
                        println!("{}", path.to_string_lossy());

                        return Ok(());
                    }
                }
            }
        }

        eprintln!("Project {} not found", name);
        Ok(())
    }

    fn project_exists(&self, name: &str) -> io::Result<bool> {
        let content = fs::read_to_string(&self.projects_db)?;
        for line in content.lines() {
            if line.starts_with(&format!("{}:", name)) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn shell() -> io::Result<String> {
    let shell = env::var("SHELL").unwrap_or_default();
    let shell_file_name = Path::new(&shell)
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to get shell file name"))?
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "Failed to convert shell file name to string",
            )
        })?;

    Ok(shell_file_name.to_string())
}

fn main() -> io::Result<()> {
    let pm = ProjectSwitch::new()?;
    pm.initialize()?;

    let cli = Cli::parse();

    match &cli.command {
        Commands::Add(args) => {
            pm.add_project(args.name.clone().unwrap_or_default())?;
        }
        Commands::Dir(args) => {
            pm.project_dir(args.name.clone())?;
        }
        Commands::Go(args) => {
            pm.go_to_project(args.name.clone())?;
        }
        Commands::Init => {
            pm.init()?;
        }
        Commands::List(args) => {
            pm.list_projects(args.raw)?;
        }
        Commands::Remove(args) => {
            pm.remove_project(&args.name)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::remove_dir_all;

    use assert_cmd::prelude::*;

    use super::*;

    #[test]
    fn test_methods() {
        let ps = ProjectSwitch::new().unwrap();
        assert!(ps.initialize().is_ok());

        let project_name = "___test_project";
        assert!(ps.add_project(project_name.to_string()).is_ok());
        assert!(ps.project_exists(project_name).unwrap());

        let project_name = "___test_project2";
        assert!(ps.add_project(project_name.to_string()).is_ok());
        assert!(ps.project_exists(project_name).unwrap());

        assert!(ps.list_projects(false).is_ok());

        let project_name = "___test_project";
        assert!(ps.remove_project(project_name).is_ok());
        assert!(!ps.project_exists(project_name).unwrap());

        let project_name = "___test_project2";
        assert!(ps.remove_project(project_name).is_ok());
        assert!(!ps.project_exists(project_name).unwrap());

        let project_name = "___test_project";
        assert!(ps.add_project(project_name.to_string()).is_ok());
        assert!(ps.project_exists(project_name).unwrap());
        assert!(ps.project_dir(project_name.to_string()).is_ok());

        assert!(remove_dir_all(ps.ps_dir).is_ok());
    }

    #[test]
    fn test_cmd() {
        let ps = ProjectSwitch::new().unwrap();
        ps.initialize().unwrap();

        let binding = Command::new(["target/debug/", assert_cmd::crate_name!()].concat());
        let mut cmd = binding;
        cmd.env("IS_TEST", "true");
        cmd.arg("add").arg("___test_project1");
        cmd.assert().stderr("Added project: ___test_project1\n");

        let binding = Command::new(["target/debug/", assert_cmd::crate_name!()].concat());
        let mut cmd = binding;
        cmd.env("IS_TEST", "true");
        cmd.arg("add").arg("___test_project2");
        cmd.assert().stderr("Added project: ___test_project2\n");

        let binding = Command::new(["target/debug/", assert_cmd::crate_name!()].concat());
        let mut cmd = binding;
        cmd.env("IS_TEST", "true");
        cmd.arg("list");
        cmd.assert().stderr("___test_project1\n___test_project2\n");

        let binding = Command::new(["target/debug/", assert_cmd::crate_name!()].concat());
        let mut cmd = binding;
        cmd.env("IS_TEST", "true");
        cmd.arg("remove").arg("___test_project1");
        cmd.assert().stderr("Removed project: ___test_project1\n");

        let binding = Command::new(["target/debug/", assert_cmd::crate_name!()].concat());
        let mut cmd = binding;
        cmd.env("IS_TEST", "true");
        cmd.arg("remove").arg("___test_project2");
        cmd.assert().stderr("Removed project: ___test_project2\n");

        assert!(remove_dir_all(ps.ps_dir).is_ok());
    }
}
