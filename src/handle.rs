use std::process::{exit, Command};

fn exec(sys_cmd: &str) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(sys_cmd)
        .spawn()
        .expect("failed to execute");

    let _ecode = child.wait().expect("failed to wait");
}

pub fn run(cmd: &str) {
    for pair in super::config::get().into_iter() {
        match pair {
            (ref k, ref v) if k == cmd => {
                exec(v);
                return;
            }
            _ => {}
        }
    }
    eprintln!("no such command <{}>", cmd);
    exit(1);
}
