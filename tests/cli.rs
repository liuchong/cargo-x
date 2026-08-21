use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
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

    fn run_with_stdin(&self, bin: &str, args: &[&str], input: &str) -> Output {
        let mut cmd = Command::new(bin);
        cmd.args(args)
            .current_dir(self.project.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn cargo_x_with_stdin(&self, args: &[&str], input: &str) -> Output {
        self.run_with_stdin(env!("CARGO_BIN_EXE_cargo-x"), args, input)
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

#[cfg(unix)]
fn append_command(file: &str, content: &str) -> String {
    format!("printf '{content}\\n' >> {file}")
}

#[cfg(windows)]
fn append_command(file: &str, content: &str) -> String {
    format!("echo {content}>> {file}")
}

#[cfg(unix)]
fn env_print_command(var: &str) -> String {
    format!("printf '%s\\n' \"${var}\"")
}

#[cfg(windows)]
fn env_print_command(var: &str) -> String {
    format!("echo %{var}%")
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

#[test]
fn no_args_lists_commands_with_desc_and_alias() {
    let fixture = Fixture::new("list");
    fixture.write_project_config(&format!(
        "{}\n[test]\ncmd = {}\ndesc = \"Run all tests\"\n\n[alias]\nt = \"test\"\n",
        command_entry("lint", &print_command("linting")),
        toml_string("cargo test"),
    ));

    let output = fixture.cargo_x(&[]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("lint"), "stdout: {out}");
    assert!(out.contains("test"), "stdout: {out}");
    assert!(out.contains("Run all tests"), "stdout: {out}");
    assert!(out.contains("-> test"), "stdout: {out}");
}

#[test]
fn list_flag_lists_commands() {
    let fixture = Fixture::new("list-flag");
    fixture.write_project_config(&command_entry("say", &print_command("hi")));

    let output = fixture.cargo_x(&["--list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("say"));
}

#[cfg(unix)]
#[test]
fn extra_args_are_appended_to_the_command() {
    let fixture = Fixture::new("args-append");
    fixture.write_project_config(&command_entry("echo", "printf '%s\\n'"));

    let output = fixture.cargo_x(&["echo", "hello", "world"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "hello\nworld");
}

#[cfg(unix)]
#[test]
fn args_placeholder_is_replaced() {
    let fixture = Fixture::new("args-placeholder");
    fixture.write_project_config(&command_entry(
        "greet",
        "printf 'hi <%s>\\n' {{args}}",
    ));

    let output = fixture.cargo_x(&["greet", "bob"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "hi <bob>");
}

#[cfg(unix)]
#[test]
fn args_with_spaces_are_quoted() {
    let fixture = Fixture::new("args-quoting");
    fixture.write_project_config(&command_entry("echo", "printf '<%s>\\n'"));

    let output = fixture.cargo_x(&["echo", "two words"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "<two words>");
}

#[test]
fn dry_run_prints_without_executing() {
    let fixture = Fixture::new("dry-run");
    fixture.write_project_config(&command_entry(
        "mk",
        &append_command("out.txt", "made"),
    ));

    let output = fixture.cargo_x(&["-n", "mk"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("out.txt"));
    assert!(!fixture.project.path().join("out.txt").exists());
}

#[test]
fn alias_runs_target_command() {
    let fixture = Fixture::new("alias");
    fixture.write_project_config(&format!(
        "{}\n[alias]\nhi = \"say\"\n",
        command_entry("say", &print_command("via-alias")),
    ));

    let output = fixture.cargo_x(&["hi"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "via-alias");
}

#[test]
fn alias_cycle_fails_with_error() {
    let fixture = Fixture::new("alias-cycle");
    fixture.write_project_config("[alias]\na = \"b\"\nb = \"a\"\n");

    let output = fixture.cargo_x(&["a"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("alias chain too long"));
}

#[test]
fn sequence_runs_steps_in_order() {
    let fixture = Fixture::new("sequence");
    fixture.write_project_config(&format!(
        "{}{}{}",
        command_entry("a", &append_command("out.txt", "a")),
        command_entry("b", &append_command("out.txt", "b")),
        "ci = [\"a\", \"b\"]\n",
    ));

    let output = fixture.cargo_x(&["ci"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out =
        fs::read_to_string(fixture.project.path().join("out.txt")).unwrap();
    assert_eq!(out.replace("\r\n", "\n").trim(), "a\nb");
}

#[test]
fn sequence_stops_on_first_failure() {
    let fixture = Fixture::new("sequence-fail");
    fixture.write_project_config(&format!(
        "{}{}{}",
        command_entry("bad", &exit_command(3)),
        command_entry("b", &append_command("out.txt", "b")),
        "ci = [\"bad\", \"b\"]\n",
    ));

    let output = fixture.cargo_x(&["ci"]);

    assert_eq!(output.status.code(), Some(3));
    assert!(!fixture.project.path().join("out.txt").exists());
}

#[test]
fn sequence_rejects_extra_args() {
    let fixture = Fixture::new("sequence-args");
    fixture.write_project_config(&format!(
        "{}{}",
        command_entry("a", &print_command("a")),
        "ci = [\"a\"]\n",
    ));

    let output = fixture.cargo_x(&["ci", "extra"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("sequence"));
}

#[test]
fn command_env_is_applied() {
    let fixture = Fixture::new("env");
    fixture.write_project_config(&format!(
        "[e]\ncmd = {}\n\n[e.env]\nX_TEST_VAR = \"from-env\"\n",
        toml_string(&env_print_command("X_TEST_VAR")),
    ));

    let output = fixture.cargo_x(&["e"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "from-env");
}

#[cfg(unix)]
#[test]
fn command_cwd_is_relative_to_config_file() {
    let fixture = Fixture::new("cwd");
    fs::create_dir_all(fixture.project.path().join("sub")).unwrap();
    fixture.write_project_config(
        "[mk]\ncmd = \"printf 'x\\n' > marker.txt\"\ncwd = \"sub\"\n",
    );

    let output = fixture.cargo_x(&["mk"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(fixture.project.path().join("sub/marker.txt").exists());
}

#[test]
fn confirm_aborts_without_approval() {
    let fixture = Fixture::new("confirm-abort");
    fixture.write_project_config(&format!(
        "[mk]\ncmd = {}\nconfirm = true\n",
        toml_string(&append_command("out.txt", "made")),
    ));

    // no stdin approval -> aborted
    let output = fixture.cargo_x(&["mk"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("aborted"));
    assert!(!fixture.project.path().join("out.txt").exists());
}

#[test]
fn confirm_runs_after_approval() {
    let fixture = Fixture::new("confirm-yes");
    fixture.write_project_config(&format!(
        "[mk]\ncmd = {}\nconfirm = true\n",
        toml_string(&append_command("out.txt", "made")),
    ));

    let output = fixture.cargo_x_with_stdin(&["mk"], "y\n");

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(fixture.project.path().join("out.txt").exists());
}

#[test]
fn init_creates_example_config_and_refuses_to_overwrite() {
    let fixture = Fixture::new("init");

    let output = fixture.cargo_x(&["--init"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let created =
        fs::read_to_string(fixture.project.path().join("x.toml")).unwrap();
    assert!(created.contains("lint"));

    let again = fixture.cargo_x(&["--init"]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("already exists"));
}

#[test]
fn show_prints_full_definition() {
    let fixture = Fixture::new("show");
    fixture.write_project_config(
        "[test]\ncmd = \"cargo test\"\ndesc = \"Run tests\"\n",
    );

    let output = fixture.cargo_x(&["--show", "test"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("cmd:     cargo test"), "stdout: {out}");
    assert!(out.contains("desc:    Run tests"), "stdout: {out}");
}

#[test]
fn comp_commands_lists_names() {
    let fixture = Fixture::new("comp-commands");
    fixture.write_project_config(&format!(
        "{}\n[alias]\nt = \"say\"\n",
        command_entry("say", &print_command("hi")),
    ));

    let output = fixture.cargo_x(&["--comp-commands"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("say"), "stdout: {out}");
    assert!(out.contains('t'), "stdout: {out}");
}

#[test]
fn completions_print_script() {
    let fixture = Fixture::new("completions");

    let output = fixture.cargo_x(&["--completions", "bash"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("complete -F _x"));
}

#[test]
fn unknown_option_fails() {
    let fixture = Fixture::new("unknown-option");

    let output = fixture.cargo_x(&["--nope"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("unknown option"));
}

#[test]
fn home_config_still_works() {
    let fixture = Fixture::new("home-config");
    fixture
        .write_home_config(&command_entry("say", &print_command("from-home")));

    let output = fixture.cargo_x(&["say"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "from-home");
}
