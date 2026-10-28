//! The old OLEGPC tasks Vitals took over (the brief, §1 p.16 and §5): the collector that wrote `health.json` and the
//! alerts. After the week next to them they go off, never deleted, by the user's own command; the panel only says
//! which are still on. Weekly and runner maintenance stay: the panel cleans only on its button.

#[cfg(windows)]
use std::time::Duration;

pub const TASKS: [&str; 2] = [r"\OLEGPC\Health collect", r"\OLEGPC\Alerts"];
#[cfg(windows)]
const DEADLINE: Duration = Duration::from_secs(10);

/// Whether the XML of `schtasks /Query /XML` says the task is on: a task turned off has `<Enabled>false</Enabled>` in
/// its settings, one on leaves it out; the `Enabled` of a trigger does not count.
pub fn is_enabled_in(xml: &str) -> bool {
    let settings = xml
        .split_once("<Settings>")
        .and_then(|(_, rest)| rest.split_once("</Settings>"))
        .map_or("", |(settings, _)| settings);
    !settings.contains("<Enabled>false</Enabled>")
}

/// The old tasks that are there and on, by their names without the leading backslash.
#[cfg(windows)]
pub fn tasks_on() -> Vec<String> {
    TASKS
        .iter()
        .filter(|task| {
            crate::process::output("schtasks", &["/Query", "/TN", task, "/XML"], DEADLINE)
                .is_ok_and(|done| done.code == 0 && is_enabled_in(&done.stdout))
        })
        .map(|task| task.trim_start_matches('\\').to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // As `schtasks /Query /TN "\OLEGPC\Alerts" /XML` prints it on OLEGPC, shortened
    const ON: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <TimeTrigger>
      <Repetition><Interval>PT15M</Interval></Repetition>
      <Enabled>false</Enabled>
    </TimeTrigger>
  </Triggers>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <ExecutionTimeLimit>PT5M</ExecutionTimeLimit>
  </Settings>
</Task>"#;

    #[test]
    fn a_task_is_on_unless_its_settings_say_off() {
        // The trigger turned off does not turn the task off
        assert!(is_enabled_in(ON));
        let off = ON.replace(
            "<ExecutionTimeLimit>",
            "<Enabled>false</Enabled>\n    <ExecutionTimeLimit>",
        );
        assert!(!is_enabled_in(&off));
        assert!(is_enabled_in(&ON.replace(
            "<ExecutionTimeLimit>",
            "<Enabled>true</Enabled><ExecutionTimeLimit>"
        )));
    }
}
