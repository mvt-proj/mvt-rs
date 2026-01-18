use crate::monitor::metrics::{PROCESS_CPU, PROCESS_MEM};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::time::{Duration, Instant, interval};

#[cfg(unix)]
fn get_cpu_time() -> Option<(f64, f64)> {
    let mut usage = std::mem::MaybeUninit::uninit();
    let res = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if res == 0 {
        let usage = unsafe { usage.assume_init() };
        let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1_000_000.0;
        let system = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1_000_000.0;
        Some((user, system))
    } else {
        None
    }
}

pub async fn start_system_monitor() {
    let pid_num = std::process::id() as usize;
    let pid = Pid::from(pid_num);

    let mut sys = System::new_all();

    let mut last_cpu_time: Option<(f64, f64)> = None;
    let mut last_wall_time = Instant::now();
    let mut warned_once = false;

    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    let mut ticker = interval(Duration::from_secs(5)); // Ajustable

    tokio::spawn(async move {
        loop {
            ticker.tick().await;

            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

            if let Some(p) = sys.process(pid) {
                let mem_bytes = p.memory();
                PROCESS_MEM.set(mem_bytes as f64);

                let cpu_from_sysinfo = p.cpu_usage() as f64;

                if cpu_from_sysinfo > 0.01 {
                    PROCESS_CPU.set(cpu_from_sysinfo);
                } else {
                    #[cfg(unix)]
                    {
                        if let Some((user, system)) = get_cpu_time() {
                            if let Some((last_user, last_system)) = last_cpu_time {
                                let now = Instant::now();
                                let wall_elapsed = now.duration_since(last_wall_time).as_secs_f64();

                                if wall_elapsed > 0.0 {
                                    let user_delta = user - last_user;
                                    let system_delta = system - last_system;
                                    let cpu_delta = user_delta + system_delta;

                                    let cpu_percent = (cpu_delta / wall_elapsed) * 100.0;
                                    PROCESS_CPU.set(cpu_percent.clamp(0.0, 100.0));
                                }
                                last_wall_time = now;
                            }
                            last_cpu_time = Some((user, system));
                        } else if !warned_once {
                            tracing::warn!("CPU metrics unavailable via libc (jail restriction?)");
                            warned_once = true;
                            PROCESS_CPU.set(0.0);
                        }
                    }

                    #[cfg(windows)]
                    {
                        PROCESS_CPU.set(0.0);
                    }
                }
            }
        }
    });
}
