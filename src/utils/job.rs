#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[cfg(windows)]
static GLOBAL_JOB: OnceLock<usize> = OnceLock::new();

#[cfg(windows)]
pub fn init_global_job() {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{CreateJobObjectW, SetInformationJobObject};
    GLOBAL_JOB.get_or_init(|| unsafe {
        let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if handle.is_null() {
            return 0usize;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            let _ = CloseHandle(handle);
            return 0usize;
        }
        handle as usize
    });
}

#[cfg(windows)]
fn global_handle() -> HANDLE {
    match GLOBAL_JOB.get() {
        Some(&value) if value != 0 => value as HANDLE,
        _ => std::ptr::null_mut(),
    }
}

#[cfg(windows)]
pub fn assign_child_to_job(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };
    let job = global_handle();
    if job.is_null() {
        return false;
    }
    let child = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid) };
    if child.is_null() {
        return false;
    }
    let assigned = unsafe {
        let result = AssignProcessToJobObject(job, child);
        let _ = CloseHandle(child);
        result
    };
    assigned != 0
}

#[cfg(windows)]
pub fn terminate_job_children() {
    use windows_sys::Win32::System::JobObjects::TerminateJobObject;
    let job = global_handle();
    if !job.is_null() {
        unsafe {
            TerminateJobObject(job, 1);
        }
    }
}

#[cfg(not(windows))]
pub fn init_global_job() {}

#[cfg(not(windows))]
pub fn assign_child_to_job(_pid: u32) -> bool {
    false
}

#[cfg(not(windows))]
pub fn terminate_job_children() {}
