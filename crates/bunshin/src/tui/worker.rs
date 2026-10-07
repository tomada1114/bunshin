//! One model thread. Ordering and all result decisions stay in core.
use bunshin_core::{
    Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, screen::ChatRequest,
};
use std::{
    io,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

enum Job {
    Probe(CancelFlag),
    Respond(ChatRequest, CancelFlag),
}
pub(super) enum Completion {
    Availability(Result<Availability, ModelError>),
    Answer(u64, Result<ModelAnswer, ModelError>),
}
pub(super) struct ModelWorker {
    jobs: Option<Sender<Job>>,
    results: Receiver<Completion>,
    thread: Option<JoinHandle<()>>,
    cancel: Option<CancelFlag>,
    busy: bool,
}
impl ModelWorker {
    pub(super) fn start(model: Arc<dyn LanguageModel>) -> io::Result<Self> {
        let (jobs, input) = mpsc::channel();
        let (output, results) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("bunshin-model".into())
            .spawn(move || {
                while let Ok(job) = input.recv() {
                    let completion = match job {
                        Job::Probe(cancel) => {
                            Completion::Availability(model.availability_with_cancel(&cancel))
                        }
                        Job::Respond(request, cancel) => {
                            Completion::Answer(request.id, model.respond(&request.request, &cancel))
                        }
                    };
                    if output.send(completion).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            results,
            thread: Some(thread),
            cancel: None,
            busy: false,
        })
    }
    pub(super) const fn busy(&self) -> bool {
        self.busy
    }
    pub(super) fn probe(&mut self) -> io::Result<()> {
        let cancel = CancelFlag::default();
        self.send(Job::Probe(cancel.clone()))?;
        self.cancel = Some(cancel);
        Ok(())
    }
    pub(super) fn respond(&mut self, request: ChatRequest) -> io::Result<()> {
        let cancel = CancelFlag::default();
        self.send(Job::Respond(request, cancel.clone()))?;
        self.cancel = Some(cancel);
        Ok(())
    }
    fn send(&mut self, job: Job) -> io::Result<()> {
        if self.busy {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let Some(sender) = &self.jobs else {
            return Err(io::ErrorKind::BrokenPipe.into());
        };
        sender
            .send(job)
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        self.busy = true;
        Ok(())
    }
    pub(super) fn poll(&mut self) -> io::Result<Option<Completion>> {
        match self.results.try_recv() {
            Ok(result) => {
                self.busy = false;
                self.cancel = None;
                Ok(Some(result))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
    pub(super) fn cancel(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
    }
    pub(super) fn shutdown(&mut self) -> io::Result<()> {
        self.cancel();
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("model worker stopped"))?;
        }
        Ok(())
    }
}

impl Drop for ModelWorker {
    fn drop(&mut self) {
        // A terminal error may unwind this owner before normal shutdown. Always cancel
        // and join; there is no surviving model child or worker after terminal restore.
        if self.shutdown().is_err() {
            tracing::error!("model worker stopped unexpectedly");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunshin_core::{ModelRequest, Tuning};
    use bunshin_test_support::ScriptedLanguageModel;
    use std::time::Duration;

    #[test]
    fn quitting_cancels_and_joins_a_running_availability_probe() {
        let (model, started) = ScriptedLanguageModel::new([]).with_probe_cancel_gate();
        let mut worker = ModelWorker::start(Arc::new(model)).expect("worker");
        worker.probe().expect("probe");
        started
            .recv_timeout(Duration::from_secs(5))
            .expect("probe entered");
        worker.shutdown().expect("cancel and join probe");
        match worker.results.recv().expect("probe completed") {
            Completion::Availability(result) => assert_eq!(result, Err(ModelError::Cancelled)),
            Completion::Answer(_, _) => panic!("expected probe"),
        }
        worker.shutdown().expect("idempotent shutdown");
    }

    #[test]
    fn cancelling_and_quitting_join_a_running_model_call_without_a_real_terminal() {
        let (model, started) = ScriptedLanguageModel::new([]).with_cancel_gate();
        let model = Arc::new(model);
        let mut worker = ModelWorker::start(model.clone()).expect("worker");
        worker
            .respond(ChatRequest {
                id: 42,
                request: ModelRequest::new(
                    "private instructions",
                    "private message",
                    "{}",
                    Tuning::default(),
                ),
            })
            .expect("start");
        started
            .recv_timeout(Duration::from_secs(5))
            .expect("call entered");
        worker.shutdown().expect("cancel and join");
        assert!(worker.thread.is_none());
        assert!(worker.jobs.is_none());
        match worker.results.recv().expect("completed after join") {
            Completion::Answer(id, result) => {
                assert_eq!(id, 42);
                assert_eq!(result, Err(ModelError::Cancelled));
            }
            Completion::Availability(_) => {
                panic!("expected response")
            }
        }
        assert_eq!(model.requests().len(), 1);
        worker.shutdown().expect("idempotent shutdown");
    }

    #[test]
    fn worker_keeps_one_request_outstanding_and_sends_ordered_answers_back_as_data() {
        let model = Arc::new(ScriptedLanguageModel::new([
            Ok(ModelAnswer {
                json: "first".into(),
            }),
            Err(ModelError::Refused),
        ]));
        let mut worker = ModelWorker::start(model.clone()).expect("worker");
        worker.probe().expect("probe");
        assert!(worker.busy());
        assert_eq!(
            worker.probe().expect_err("one job").kind(),
            io::ErrorKind::WouldBlock
        );
        match worker.results.recv().expect("probe result") {
            Completion::Availability(result) => assert_eq!(result, Ok(Availability::Available)),
            Completion::Answer(_, _) => panic!("expected probe"),
        }
        worker.busy = false;
        for id in [1, 2] {
            worker
                .respond(ChatRequest {
                    id,
                    request: ModelRequest::new("rules", "input", "{}", Tuning::default()),
                })
                .expect("request");
            match worker.results.recv().expect("response") {
                Completion::Answer(token, result) => {
                    assert_eq!(token, id);
                    if id == 1 {
                        assert_eq!(
                            result,
                            Ok(ModelAnswer {
                                json: "first".into()
                            })
                        );
                    } else {
                        assert_eq!(result, Err(ModelError::Refused));
                    }
                }
                Completion::Availability(_) => {
                    panic!("expected answer")
                }
            }
            worker.busy = false;
        }
        assert_eq!(model.requests().len(), 2);
        worker.shutdown().expect("joined");
    }
}
