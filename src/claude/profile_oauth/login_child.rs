//! Bind the hidden login process to a kill-on-close job at creation time.
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::ExitStatusExt,
    },
    process::{Command, ExitStatus},
};
use windows_sys::Win32::{
    Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
            INFINITE, InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
            STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
};

pub(super) struct LoginChild {
    process: OwnedHandle,
    job: OwnedHandle,
}

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut value: Vec<_> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in login command",
        ));
    }
    value.push(0);
    Ok(value)
}

struct Attributes(Vec<usize>);
impl Attributes {
    fn pointer(&mut self) -> *mut std::ffi::c_void {
        self.0.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            // SAFETY: only successfully initialized lists retain their allocation.
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

impl LoginChild {
    pub(super) fn spawn(command: &Command) -> io::Result<Self> {
        let application = wide(command.get_program())?;
        // The only arguments are fixed CLI words. Paths are passed separately;
        // quote argv[0] so spaces in the native executable path remain intact.
        let mut command_line = vec![b'"' as u16];
        command_line.extend_from_slice(&application[..application.len() - 1]);
        command_line.extend("\" auth login --claudeai".encode_utf16());
        command_line.push(0);
        let directory = wide(
            command
                .get_current_dir()
                .ok_or_else(|| io::Error::other("Missing login directory"))?
                .as_os_str(),
        )?;
        let mut environment = BTreeMap::new();
        for (key, value) in std::env::vars_os() {
            environment.insert(key.to_string_lossy().to_uppercase(), (key, value));
        }
        for (key, value) in command.get_envs() {
            let normalized = key.to_string_lossy().to_uppercase();
            if let Some(value) = value {
                environment.insert(normalized, (key.to_owned(), value.to_owned()));
            } else {
                environment.remove(&normalized);
            }
        }
        let mut block = Vec::new();
        for (_, (key, value)) in environment {
            block.extend(key.encode_wide());
            block.push(b'=' as u16);
            block.extend(value.encode_wide());
            block.push(0);
        }
        block.push(0);
        let nul = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("NUL")?;
        // SAFETY: owned job/process handles close on every error. Only the NUL
        // stdio handle is inherited; the job handle remains in the parent.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = OwnedHandle::from_raw_handle(job);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let mut bytes = 0;
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut bytes);
            if bytes == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut attributes = Attributes(vec![0; bytes.div_ceil(std::mem::size_of::<usize>())]);
            if InitializeProcThreadAttributeList(attributes.pointer(), 2, 0, &mut bytes) == 0 {
                attributes.0.clear();
                return Err(io::Error::last_os_error());
            }
            let jobs = [job.as_raw_handle()];
            if UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                jobs.as_ptr().cast(),
                std::mem::size_of_val(&jobs),
                std::ptr::null_mut(),
                std::ptr::null(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            if SetHandleInformation(
                nul.as_raw_handle(),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let handles = [nul.as_raw_handle()];
            if UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast(),
                std::mem::size_of_val(&handles),
                std::ptr::null_mut(),
                std::ptr::null(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let mut startup: STARTUPINFOEXW = std::mem::zeroed();
            startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = nul.as_raw_handle();
            startup.StartupInfo.hStdOutput = nul.as_raw_handle();
            startup.StartupInfo.hStdError = nul.as_raw_handle();
            startup.lpAttributeList = attributes.pointer();
            let mut process: PROCESS_INFORMATION = std::mem::zeroed();
            // No captured output. The CLI opens the browser;
            // callback authorization is its responsibility, not Minibar's.
            if CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
                block.as_ptr().cast(),
                directory.as_ptr(),
                &startup.StartupInfo,
                &mut process,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let _thread = OwnedHandle::from_raw_handle(process.hThread);
            Ok(Self {
                process: OwnedHandle::from_raw_handle(process.hProcess),
                job,
            })
        }
    }
    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        // SAFETY: process remains owned and valid.
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => self.status().map(Some),
            _ => Err(io::Error::last_os_error()),
        }
    }
    fn status(&self) -> io::Result<ExitStatus> {
        let mut code = 0;
        // SAFETY: process and output storage are valid.
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ExitStatus::from_raw(code))
    }
    pub(super) fn kill(&mut self) -> io::Result<()> {
        // SAFETY: private job contains only the login process and descendants.
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        // SAFETY: process remains owned while waiting.
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), INFINITE) } != WAIT_OBJECT_0 {
            return Err(io::Error::last_os_error());
        }
        self.status()
    }
}
impl Drop for LoginChild {
    fn drop(&mut self) {
        let _ = self.kill();
        let _ = self.wait();
    }
}
