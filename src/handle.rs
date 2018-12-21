use super::sys_cfg::{SHELL_ARG, SHELL_CMD};
use std::process::{Command, ExitStatus};

fn exec(sys_cmd: &str) -> ExitStatus {
    let mut child = Command::new(SHELL_CMD)
        .arg(SHELL_ARG)
        .arg(sys_cmd)
        .spawn()
        .expect("failed to execute");

    child.wait().expect("failed to wait")
}

pub fn run(cmd: &str) -> Option<i32> {
    for pair in super::config::get().into_iter() {
        match pair {
            (ref k, ref v) if k == cmd => {
                return exec(v).code();
            }
            _ => {}
        }
    }
    eprintln!("no such command <{}>", cmd);
    Some(1)
}
