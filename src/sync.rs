use crate::scheduler::SCHEDULER;

pub fn yield_thread() {
    unsafe {
        let scheduler = SCHEDULER.get();

        if let Some(current_id) = scheduler.get_current_thread() {
            if let Some(next_id) = scheduler.schedule() {
                if current_id != next_id {
                    scheduler.set_current_thread(Some(next_id));
                    let _ = scheduler.switch_context(current_id, next_id);
                }
            }
        }
    }
}

pub fn exit_thread() -> ! {
    unsafe {
        let scheduler = SCHEDULER.get();
        
        if let Some(current_id) = scheduler.get_current_thread() {
            scheduler.exit_current_thread();

            if let Some(next_id) = scheduler.schedule() {
                scheduler.set_current_thread(Some(next_id));
                let _ = scheduler.switch_context(current_id, next_id);
            }
        }
    }

    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("hlt") }
        
        #[cfg(not(target_arch = "x86_64"))]
        core::hint::spin_loop();
    }
}

pub fn sleep_ms(_ms: u64) {
    yield_thread();
}
