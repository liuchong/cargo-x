use super::config::Xconf;
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

pub fn run(cmd: &str, x_conf: Xconf) -> Option<i32> {
    for pair in x_conf.into_iter() {
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
