use super::job_policy::*;
use super::process::Process;
use crate::object::*;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

pub struct Job {
    base: KObjectBase,
    parent: Option<Arc<Job>>,
    inner: Mutex<JobInner>,
}

impl_kobject!(Job);

#[derive(Default)]
struct JobInner {
    policy: JobPolicy,
    children: Vec<Arc<Job>>,
    processes: Vec<Arc<Process>>,
}

lazy_static! {
    /// The root job
    pub static ref ROOT_JOB: Arc<Job> = Arc::new(Job {
        base: KObjectBase::new(),
        parent: None,
        inner: Mutex::new(JobInner::default()),
    });
}

impl Job {
    /// Create a new child job object.
    pub fn create_child(parent: &Arc<Self>, _options: u32) -> ZxResult<Arc<Self>> {
        // TODO: options
        let child = Arc::new(Job {
            base: KObjectBase::new(),
            parent: Some(parent.clone()),
            inner: Mutex::new(JobInner::default()),
        });
        parent.inner.lock().children.push(child.clone());
        Ok(child)
    }

    pub fn policy(&self) -> JobPolicy {
        self.inner.lock().policy.clone()
    }

    /// Sets one or more security and/or resource policies to an empty job.
    ///
    /// The job's effective policies is the combination of the parent's
    /// effective policies and the policies specified in policy.
    ///
    /// After this call succeeds any new child process or child job will have
    /// the new effective policy applied to it.
    pub fn set_policy_basic(&self, _options: SetPolicyOptions, _policys: &[BasicPolicy]) {
        unimplemented!()
    }

    pub fn set_policy_timer_slack(
        &self,
        _options: SetPolicyOptions,
        _policys: &[TimerSlackPolicy],
    ) {
        unimplemented!()
    }

    /// Add a process to the job.
    pub(super) fn add_process(&self, process: Arc<Process>) {
        self.inner.lock().processes.push(process);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create() {
        let job = Job::create_child(&ROOT_JOB, 0).expect("failed to create job");
    }
}
