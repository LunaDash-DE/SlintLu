//! The `System` and `Battery` globals, polled every two seconds.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::system::{self, BatteryState, CpuSampler, NetworkSampler};
use slintlu::util::format;

use crate::scripting::ShellHost;
use crate::scripting::bridge::{list_value, string, struct_value};

const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Samplers {
    cpu: CpuSampler,
    network: NetworkSampler,
}

fn number(value: impl Into<f64>) -> Value {
    Value::Number(value.into())
}

fn poll(host: &ShellHost, samplers: &mut Samplers) {
    let (cpu, cores) = samplers.cpu.sample();
    host.set_global("System", "cpu", number(cpu));
    host.set_global("System", "cpu-cores", list_value(cores.into_iter().map(number)));

    let memory = system::memory();
    host.set_global("System", "memory", number(memory.usage()));
    host.set_global("System", "swap", number(memory.swap_usage()));
    host.set_global(
        "System",
        "memory-text",
        string(format!("{} / {}", format::bytes(memory.used()), format::bytes(memory.total))),
    );

    if let Some(disk) = system::disk_usage(Path::new("/")) {
        host.set_global("System", "disk", number(disk.usage()));
        host.set_global(
            "System",
            "disk-text",
            string(format!("{} free", format::bytes(disk.available))),
        );
    }

    let temperatures = system::temperatures();
    let hottest = system::headline_temperature(&temperatures);
    host.set_global("System", "temperature", number(hottest));
    host.set_global(
        "System",
        "temperatures",
        list_value(
            temperatures.iter().map(|t| {
                struct_value([("label", string(&t.label)), ("celsius", number(t.celsius))])
            }),
        ),
    );

    let traffic = samplers.network.sample();
    host.set_global("System", "download", number(traffic.download));
    host.set_global("System", "upload", number(traffic.upload));
    host.set_global("System", "download-text", string(format::rate(traffic.download)));
    host.set_global("System", "upload-text", string(format::rate(traffic.upload)));
    host.set_global("System", "uptime", string(format::duration(system::uptime())));

    let battery = system::batteries().into_iter().next();
    host.set_global("Battery", "available", Value::Bool(battery.is_some()));
    if let Some(battery) = battery {
        host.set_global("Battery", "level", number(battery.level));
        host.set_global(
            "Battery",
            "charging",
            Value::Bool(battery.state == BatteryState::Charging),
        );
        host.set_global("Battery", "state", string(battery.state.as_str()));
        host.set_global(
            "Battery",
            "time-remaining",
            string(
                battery
                    .time_remaining
                    .map(|s| format::duration(Duration::from_secs(s)))
                    .unwrap_or_default(),
            ),
        );
    }
}

pub fn install(host: &Rc<ShellHost>) {
    let info = system::host_info();
    host.set_global("System", "hostname", string(&info.hostname));
    host.set_global("System", "kernel", string(&info.kernel));
    host.set_global("System", "os", string(&info.os));

    let samplers = RefCell::new(Samplers::default());
    poll(host, &mut samplers.borrow_mut());
    let weak = Rc::downgrade(host);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, INTERVAL, move || {
        if let Some(host) = weak.upgrade() {
            poll(&host, &mut samplers.borrow_mut());
        }
    });
    // Polling lasts as long as the process.
    std::mem::forget(timer);
}
