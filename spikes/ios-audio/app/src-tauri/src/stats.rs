//! Нагрузка процесса: CPU (getrusage) и память (phys_footprint — то, что iOS считает для
//! лимита памяти и показывает Xcode).

use std::sync::Mutex;
use std::time::Instant;

pub struct Sample {
    pub cpu_percent: f64,
    pub footprint_mb: f64,
    pub peak_footprint_mb: f64,
}

static LAST: Mutex<Option<(Instant, f64)>> = Mutex::new(None);

fn cpu_seconds() -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}

#[cfg(target_vendor = "apple")]
fn footprint() -> (f64, f64) {
    use mach2::task_info::{TASK_VM_INFO, task_vm_info};
    let mut info = task_vm_info::default();
    let mut count = (std::mem::size_of::<task_vm_info>()
        / std::mem::size_of::<mach2::vm_types::natural_t>()) as u32;
    let kr = unsafe {
        mach2::task::task_info(
            mach2::traps::mach_task_self(),
            TASK_VM_INFO,
            &mut info as *mut task_vm_info as *mut i32,
            &mut count,
        )
    };
    if kr != 0 {
        return (0.0, 0.0);
    }
    let mb = |v: u64| v as f64 / (1024.0 * 1024.0);
    (
        mb(info.phys_footprint),
        mb(info.ledger_phys_footprint_peak as u64),
    )
}

#[cfg(not(target_vendor = "apple"))]
fn footprint() -> (f64, f64) {
    (0.0, 0.0)
}

/// CPU — средний за время с прошлого вызова (100 % = одно ядро).
pub fn sample() -> Sample {
    let now = Instant::now();
    let cpu = cpu_seconds();
    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    let cpu_percent = match *last {
        Some((t, c)) if now > t => (cpu - c) / now.duration_since(t).as_secs_f64() * 100.0,
        _ => 0.0,
    };
    *last = Some((now, cpu));
    let (fp, peak) = footprint();
    Sample {
        cpu_percent,
        footprint_mb: fp,
        peak_footprint_mb: peak,
    }
}
