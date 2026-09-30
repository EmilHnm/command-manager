//! Keep a memory-hungry child from taking the whole app down with it.
//!
//! systemd-oomd kills whole cgroups, and a GUI app launched from the desktop
//! shares one `app-*.scope` with every child it spawns, so one runaway
//! terminal used to get the app and all other terminals killed. Like GNOME
//! Terminal (`vte-spawn-*.scope`), each child is moved into its own transient
//! scope so oomd can pick that terminal alone. The kernel OOM killer works
//! per process instead; a higher `oom_score_adj` makes it prefer the child.

use std::sync::OnceLock;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

/// Added to the app's own `oom_score_adj` for every child (max 1000).
const CHILD_OOM_SCORE_BONUS: i32 = 300;

pub fn isolate_child(pid: u32) {
    raise_oom_score_adj(pid);
    // A D-Bus call can wait up to its timeout on a stuck bus; never make
    // opening a terminal wait for it.
    std::thread::spawn(move || {
        if let Err(error) = move_to_own_scope(pid) {
            eprintln!("Command Manager: could not move child {pid} to its own scope: {error}");
        }
    });
}

fn read_oom_score_adj(path: &str) -> Option<i32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn raise_oom_score_adj(pid: u32) {
    let own = read_oom_score_adj("/proc/self/oom_score_adj").unwrap_or(0);
    let target = own.saturating_add(CHILD_OOM_SCORE_BONUS).min(1000);
    let path = format!("/proc/{pid}/oom_score_adj");
    // Raising the value needs no privilege; lowering it would.
    if read_oom_score_adj(&path).is_some_and(|current| current < target) {
        if let Err(error) = std::fs::write(&path, target.to_string()) {
            eprintln!("Command Manager: could not raise oom_score_adj of {pid}: {error}");
        }
    }
}

fn session_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| Connection::session().ok()).as_ref()
}

/// The app's own unit and slice when it runs under the systemd user manager,
/// e.g. `app-gnome-Command\x20Manager-603520.scope` in `app.slice`.
fn own_user_unit() -> Option<(String, Option<String>)> {
    let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = cgroup.lines().find_map(|line| line.strip_prefix("0::"))?;
    let (_, below_manager) = path.split_once("/user@")?;
    // "1000.service/app.slice/<unit>"; the manager's own init.scope is not
    // a unit the app can bind to.
    let mut segments = below_manager
        .split('/')
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev();
    let unit = segments.next().filter(|name| {
        *name != "init.scope" && (name.ends_with(".scope") || name.ends_with(".service"))
    })?;
    let slice = segments
        .next()
        .filter(|name| name.ends_with(".slice"))
        .map(str::to_string);
    Some((unit.to_string(), slice))
}

fn move_to_own_scope(pid: u32) -> zbus::Result<()> {
    let Some(bus) = session_bus() else {
        return Ok(());
    };
    let name = format!(
        "app-command\\x2dmanager-{}.scope",
        uuid::Uuid::new_v4().simple()
    );
    let description = format!("Command Manager child process {pid}");
    let base = vec![
        ("PIDs", Value::from(vec![pid])),
        ("Description", Value::from(description.as_str())),
        ("CollectMode", Value::from("inactive-or-failed")),
    ];
    // Stay next to the app in its slice and stop with it, so a child that
    // ignores SIGHUP is not orphaned when the app itself is killed.
    let mut bound = base.clone();
    let own = own_user_unit();
    if let Some((unit, slice)) = &own {
        bound.push(("BindsTo", Value::from(vec![unit.as_str()])));
        bound.push(("After", Value::from(vec![unit.as_str()])));
        if let Some(slice) = slice {
            bound.push(("Slice", Value::from(slice.as_str())));
        }
    }
    let start = |properties: &Vec<(&str, Value<'_>)>| {
        let aux: Vec<(&str, Vec<(&str, Value<'_>)>)> = Vec::new();
        bus.call_method(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            Some("org.freedesktop.systemd1.Manager"),
            "StartTransientUnit",
            &(name.as_str(), "fail", properties, aux),
        )
        .map(|_| ())
    };
    match start(&bound) {
        Err(_) if own.is_some() => start(&base),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[test]
    fn child_gets_a_higher_oom_score_and_its_own_scope() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        super::isolate_child(pid);

        let own = super::read_oom_score_adj("/proc/self/oom_score_adj").unwrap_or(0);
        let adj = super::read_oom_score_adj(&format!("/proc/{pid}/oom_score_adj"));
        let expected = (own + super::CHILD_OOM_SCORE_BONUS).min(1000);

        // Only a desktop session has a systemd user manager on the bus.
        let mut scope = None;
        if super::session_bus().is_some() && super::own_user_unit().is_some() {
            for _ in 0..100 {
                let cgroup =
                    std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap_or_default();
                if cgroup.contains("app-command\\x2dmanager-") {
                    scope = Some(cgroup);
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(scope.is_some(), "child was not moved to its own scope");
        }
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(adj, Some(expected));
    }
}
