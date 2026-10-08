//! One model thread. Ordering and all result decisions stay in core.
use bunshin_core::{CancelFlag, LanguageModel, ModelAnswer, ModelError, screen::BoardRequest};
use std::{
    io,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

enum Job {
    Respond(BoardRequest, CancelFlag),
}

pub(super) enum Completion {
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
                while let Ok(Job::Respond(request, cancel)) = input.recv() {
                    let completion =
                        Completion::Answer(request.id, model.respond(&request.request, &cancel));
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

    pub(super) fn respond(&mut self, request: BoardRequest) -> io::Result<()> {
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

    fn request(id: u64) -> BoardRequest {
        BoardRequest {
            id,
            request: ModelRequest::new("private instructions", "private prompt", Tuning::default()),
        }
    }

    #[test]
    fn quitting_cancels_and_joins_a_running_model_call_without_a_real_terminal() {
        let (model, started) = ScriptedLanguageModel::new([]).with_cancel_gate();
        let mut worker = ModelWorker::start(Arc::new(model)).expect("worker");
        worker.respond(request(42)).expect("start model call");
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
        }
        worker.shutdown().expect("idempotent shutdown");
    }

    #[test]
    fn worker_keeps_one_request_outstanding_and_sends_ordered_answers_back_as_data() {
        let model = Arc::new(ScriptedLanguageModel::new([
            Ok(ModelAnswer {
                text: "first".into(),
            }),
            Err(ModelError::Refused),
        ]));
        let mut worker = ModelWorker::start(model.clone()).expect("worker");
        worker.respond(request(1)).expect("start first");
        assert!(worker.busy());
        assert_eq!(
            worker.respond(request(2)).expect_err("only one job").kind(),
            io::ErrorKind::WouldBlock
        );
        match worker.results.recv().expect("first response") {
            Completion::Answer(id, result) => {
                assert_eq!(id, 1);
                assert_eq!(
                    result,
                    Ok(ModelAnswer {
                        text: "first".into()
                    })
                );
            }
        }
        worker.busy = false;
        worker.respond(request(2)).expect("start second");
        match worker.results.recv().expect("second response") {
            Completion::Answer(id, result) => {
                assert_eq!(id, 2);
                assert_eq!(result, Err(ModelError::Refused));
            }
        }
        assert_eq!(model.requests().len(), 2);
        worker.shutdown().expect("joined");
    }
}
