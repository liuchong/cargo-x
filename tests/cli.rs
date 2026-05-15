use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir()
            .join(format!("cargo-x-it-{}-{label}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Fixture {
    home: TempDir,
    project: TempDir,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let home = TempDir::new(&format!("{label}-home"));
        let project = TempDir::new(&format!("{label}-project"));
        fs::create_dir_all(project.path().join("src")).unwrap();
        fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(project.path().join("src/lib.rs"), "").unwrap();

        Self { home, project }
    }

    fn write_home_config(&self, contents: &str) {
        fs::write(self.home.path().join(".x.toml"), contents).unwrap();
    }

    fn write_project_config(&self, contents: &str) {
        fs::write(self.project.path().join("x.toml"), contents).unwrap();
    }

    fn write_manifest(&self, metadata: &str) {
        fs::write(
            self.project.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n{metadata}"
            ),
        )
        .unwrap();
    }

    fn cargo_x(&self, args: &[&str]) -> Output {
        self.run(env!("CARGO_BIN_EXE_cargo-x"), args)
    }

    fn x(&self, args: &[&str]) -> Output {
        self.run(env!("CARGO_BIN_EXE_x"), args)
    }

    fn run(&self, bin: &str, args: &[&str]) -> Output {
        let mut cmd = Command::new(bin);
        cmd.args(args)
            .current_dir(self.project.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path());

        cmd.output().unwrap()
    }
}

#[cfg(unix)]
fn print_command(message: &str) -> String {
    format!("printf '{message}\\n'")
}

#[cfg(windows)]
fn print_command(message: &str) -> String {
    format!("echo {message}")
}

#[cfg(unix)]
fn exit_command(code: i32) -> String {
    format!("exit {code}")
}

#[cfg(windows)]
fn exit_command(code: i32) -> String {
    format!("exit /B {code}")
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn command_entry(name: &str, command: &str) -> String {
    format!("{name} = {}\n", toml_string(command))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

#[test]
fn cargo_x_runs_command_from_project_config() {
    let fixture = Fixture::new("project-config");
    fixture.write_project_config(&command_entry(
        "say",
        &print_command("from-project"),
    ));

    let output = fixture.cargo_x(&["say"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "from-project");
}

#[test]
fn cargo_subcommand_form_skips_the_extra_x_arg() {
    let fixture = Fixture::new("cargo-subcommand");
    fixture.write_project_config(&command_entry(
        "say",
        &print_command("from-cargo-x"),
    ));

    let output = fixture.cargo_x(&["x", "say"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "from-cargo-x");
}

#[test]
fn x_binary_runs_command_from_project_config() {
    let fixture = Fixture::new("x-binary");
    fixture
        .write_project_config(&command_entry("say", &print_command("from-x")));

    let output = fixture.x(&["say"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "from-x");
}

#[test]
fn manifest_metadata_overrides_project_and_home_config() {
    let fixture = Fixture::new("precedence");
    fixture.write_home_config(&command_entry(
        "shared",
        &print_command("from-home"),
    ));
    fixture.write_project_config(&command_entry(
        "shared",
        &print_command("from-project"),
    ));
    fixture.write_manifest(&format!(
        "[package.metadata.x]\n{}",
        command_entry("shared", &print_command("from-manifest"))
    ));

    let output = fixture.cargo_x(&["shared"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "from-manifest");
}

#[test]
fn missing_command_fails_with_error_message() {
    let fixture = Fixture::new("missing-command");
    fixture.write_project_config(&command_entry(
        "say",
        &print_command("available"),
    ));

    let output = fixture.cargo_x(&["missing"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no such command <missing>"));
}

#[test]
fn configured_command_exit_status_is_propagated() {
    let fixture = Fixture::new("exit-status");
    fixture.write_project_config(&command_entry("fail", &exit_command(7)));

    let output = fixture.cargo_x(&["fail"]);

    assert_eq!(output.status.code(), Some(7));
}
